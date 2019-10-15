use crate::ssh;
use failure::Error;
use std::collections::HashMap;
use std::sync::Arc;

/// Descriptor for a single, existing machine to connect to.
/// Therefore, the `impl MachineSetup` includes the address of the machine in `region`; i.e.,
/// each instance of Setup corresponds to a single machine.
#[derive(Clone)]
pub struct Setup {
    addr: Vec<std::net::SocketAddr>,
    username: String,
    key_path: Option<std::path::PathBuf>,
    setup_fn:
        Option<Arc<dyn Fn(&mut ssh::Session, &slog::Logger) -> Result<(), Error> + Send + Sync>>,
}

impl super::MachineSetup for Setup {
    type Region = String;
    fn region(&self) -> Self::Region {
        format!("bare:{}", self.addr[0])
    }
}

impl Setup {
    pub fn new(
        addr: impl std::net::ToSocketAddrs,
        username: Option<String>,
    ) -> Result<Self, Error> {
        let username: Result<String, Error> = username.map(Ok).unwrap_or_else(|| {
            let user = String::from_utf8(std::process::Command::new("whoami").output()?.stdout)?;
            let user = user
                .split_whitespace()
                .next()
                .expect("expect newline after whoami output");
            Ok(user.to_string())
        });
        let username = username?;
        let mut addr: Vec<std::net::SocketAddr> = addr.to_socket_addrs()?.collect();
        addr.reverse(); // so pop() will reutrn in the same order

        Ok(Self {
            username,
            addr,
            key_path: None,
            setup_fn: None,
        })
    }

    pub fn key_path(self, p: impl AsRef<std::path::Path>) -> Self {
        Self {
            key_path: Some(p.as_ref().to_path_buf()),
            ..self
        }
    }

    pub fn setup(
        self,
        setup: impl Fn(&mut ssh::Session, &slog::Logger) -> Result<(), Error> + Send + Sync + 'static,
    ) -> Self {
        Self {
            setup_fn: Some(Arc::new(setup)),
            ..self
        }
    }
}

fn try_addrs(
    s: &mut Setup,
    log: &slog::Logger,
    max_wait: Option<std::time::Duration>,
) -> Result<std::net::SocketAddr, Error> {
    use failure::ResultExt;
    let mut err = Err(format_err!("SSH failed")).context(String::from("No valid addresses found"));
    while let Some(addr) = s.addr.pop() {
        match ssh::Session::connect(
            log,
            &s.username,
            addr,
            s.key_path.as_ref().map(|p| p.as_path()),
            max_wait,
        ) {
            Err(e) => {
                trace!(log, "failed to ssh to addr {}", &addr; "err" => ?e);
                err = err.context(format!("failed to ssh to address {}", addr))
            }
            Ok(_) => {
                return Ok(addr);
            }
        }
    }

    err?
}

/// Only one machine is supported per instance of this Launcher, further instances of `Setup`
/// passed to `launch` will
/// be ignored, since it doesn't make sense to connect to the same machine twice.
///
/// The `impl Drop` of this type is a no-op, since Tsunami can't terminate an existing machine.
#[derive(Default)]
pub struct Machine {
    pub log: Option<slog::Logger>,
    name: String,
    addr: Option<std::net::SocketAddr>,
    username: String,
    key_path: Option<std::path::PathBuf>,
}

impl super::Launcher for Machine {
    type MachineDescriptor = Setup;

    fn launch(&mut self, l: super::LaunchDescriptor<Self::MachineDescriptor>) -> Result<(), Error> {
        self.log = Some(l.log);
        let log = self.log.as_ref().expect("Baremetal machine uninitialized");

        let mut dscs = l.machines.into_iter();
        let (name, mut setup) = dscs
            .next()
            .ok_or_else(|| format_err!("Cannot initialize zero machines"))?;
        for (discarded_name, discarded_setup) in dscs {
            warn!(log, "Discarding duplicate connections to same machine";
                "name" => &discarded_name,
                "addr" => &discarded_setup.addr[0],
            );
        }

        let addr = try_addrs(&mut setup, &log, l.max_wait)?;

        if let Setup {
            setup_fn: Some(f), ..
        } = setup
        {
            let mut sess = ssh::Session::connect(
                log,
                &setup.username,
                addr,
                setup.key_path.as_ref().map(|p| p.as_path()),
                l.max_wait,
            )
            .map_err(|e| {
                error!(log, "failed to ssh to {}", &addr);
                e.context(format!("failed to ssh to machine {}", addr))
            })?;

            f(&mut sess, log).map_err(|e| {
                error!(
                    log,
                    "machine setup failed";
                    "name" => name.clone(),
                );
                e.context(format!("setup procedure for {} machine failed", name))
            })?;
        }

        info!(log, "finished setting up instance"; "name" => &name, "ip" => &addr);
        self.name = name;
        self.addr = Some(addr);
        self.username = setup.username;
        self.key_path = setup.key_path;
        Ok(())
    }

    fn connect_all<'l>(&'l self) -> Result<HashMap<String, crate::Machine<'l>>, Error> {
        let log = self.log.as_ref().expect("Baremetal machine uninitialized");
        let addr = self
            .addr
            .ok_or_else(|| format_err!("Address uninitialized"))?;
        let sess = ssh::Session::connect(
            log,
            &self.username,
            addr,
            self.key_path.as_ref().map(|p| p.as_path()),
            None,
        )
        .map_err(|e| {
            error!(log, "failed to ssh to {}", &addr);
            e.context(format!("failed to ssh to machine {}", addr))
        })?;

        let mut hmap: HashMap<String, crate::Machine<'l>> = Default::default();
        hmap.insert(
            self.name.clone(),
            crate::Machine {
                nickname: self.name.clone(),
                public_dns: addr.to_string(),
                public_ip: addr.ip().to_string(),
                ssh: Some(sess),
                _tsunami: Default::default(),
            },
        );
        Ok(hmap)
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        let log = self.log.as_ref().expect("Baremetal machine uninitialized");
        debug!(log, "Dropping baremetal machine"; "addr" => self.addr.unwrap());
    }
}

#[cfg(test)]
mod test {
    use crate::providers::Launcher;
    use failure::Error;

    #[test]
    #[ignore]
    fn localhost() -> Result<(), Error> {
        let s = super::Setup::new("127.0.0.1:22", None)?;
        let mut m: super::Machine = Default::default();
        m.log = Some(crate::test::test_logger());
        let desc = crate::providers::LaunchDescriptor {
            region: String::from("localhost"),
            log: crate::test::test_logger(),
            max_wait: None,
            machines: vec![(String::from("self"), s)],
        };
        m.launch(desc)?;
        let ms = m.connect_all()?;
        ms.get("self")
            .unwrap()
            .ssh
            .as_ref()
            .unwrap()
            .cmd("ls")
            .unwrap();
        Ok(())
    }
}
