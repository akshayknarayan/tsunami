//! Implements backend functionality to spawn machines.

use failure::Error;
use itertools::Itertools;
use std::collections::HashMap;

/// A description of a set of machines to launch.
///
/// The machines are constrained to a single `region`.
#[derive(Debug)]
pub struct LaunchDescriptor<M: MachineSetup> {
    /// The region to launch into.
    pub region: M::Region,
    /// A logger.
    pub log: slog::Logger,
    /// An optional timeout.
    ///
    /// If specified and the LaunchDescriptor is not launched in the given time,
    /// [`crate::TsunamiBuilder::spawn`] will fail with an error.
    pub max_wait: Option<std::time::Duration>,
    /// The machines to launch.
    pub machines: Vec<(String, M)>,
}

/// This is used to group machines into connections
/// to cloud providers. e.g., for AWS we need a separate
/// connection to each region.
pub trait MachineSetup {
    /// Grouping type.
    type Region: Eq + std::hash::Hash + Clone + ToString;
    /// Get the region.
    fn region(&self) -> Self::Region;
}

/// Implement this trait to implement a new cloud provider for Tsunami.
/// Tsunami will call `launch` once per unique region, as defined by `MachineSetup`.
pub trait Launcher {
    /// A type describing a single instance to launch.
    type MachineDescriptor: MachineSetup;

    /// Spawn the instances.
    ///
    /// Implementors should remember enough information to subsequently answer
    /// calls to `connect_all`, i.e., the IPs of the machines.
    ///
    /// This method can be called multiple times. Subsequent calls to
    /// `connect_all` should return the new machines as well as any previously
    /// spawned machines.
    fn launch(&mut self, desc: LaunchDescriptor<Self::MachineDescriptor>) -> Result<(), Error>;

    /// Return connections to the [`Machine`s](crate::Machine) that `launch` spawned.
    fn connect_all<'l>(&'l self) -> Result<HashMap<String, crate::Machine<'l>>, Error>;

    /// Start up all the hosts.
    ///
    /// This call will block until the instances are spawned into the provided launcher.
    /// SSH connections to each instance are accesssible via
    /// [`connect_all`](providers::Launcher::connect_all).
    ///
    /// # Arguments
    /// - `descriptors` is an iterator of machine nickname to descriptor. Duplicate nicknames will
    /// cause an error. To add many and auto-generate nicknames, see the helper function
    /// [`crate::make_multiple`].
    /// - `max_wait` limits how long we should wait for instances to be available before giving up.
    /// Passing `None` implies no limit.
    ///
    /// # Example
    /// ```rust,no_run
    /// fn main() -> Result<(), failure::Error> {
    ///     use tsunami::providers::Launcher;
    ///     // make a launcher
    ///     let mut aws: tsunami::providers::aws::Launcher<_> = Default::default();
    ///     // spawn hosts into the launcher
    ///     aws.spawn(vec![(String::from("my_tsunami"), Default::default())], None, None)?;
    ///     // access hosts via the launcher
    ///     let vms = aws.connect_all()?;
    ///     Ok(())
    /// }
    /// ```
    fn spawn(
        &mut self,
        descriptors: impl IntoIterator<Item = (String, Self::MachineDescriptor)>,
        max_wait: Option<std::time::Duration>,
        log: Option<slog::Logger>,
    ) -> Result<(), Error> {
        let max_wait = max_wait;
        let log = log.unwrap_or_else(|| slog::Logger::root(slog::Discard, o!()));

        info!(log, "spinning up tsunami");

        for (region_name, setups) in descriptors
            .into_iter()
            .map(|(name, setup)| (setup.region(), (name, setup)))
            .into_group_map()
        {
            let region_log = log.new(slog::o!("region" => region_name.clone().to_string()));
            let dsc = LaunchDescriptor {
                region: region_name.clone(),
                log: region_log,
                max_wait,
                machines: setups,
            };

            self.launch(dsc)?;
        }

        Ok(())
    }
}

macro_rules! collect {
    ($x: expr) => {{
        $x.values()
            .map(|r| r.connect_all())
            .fold(Ok(HashMap::default()), |acc, el| {
                acc.and_then(|mut a| {
                    a.extend(el?.into_iter());
                    Ok(a)
                })
            })
    }};
}

struct Sep(&'static str);

impl Default for Sep {
    fn default() -> Self {
        Sep("_")
    }
}

impl From<&'static str> for Sep {
    fn from(s: &'static str) -> Self {
        Sep(s)
    }
}

fn rand_name(prefix: &str) -> String {
    rand_name_sep(prefix, "_")
}

fn rand_name_sep(prefix: &str, sep: impl Into<Sep>) -> String {
    use rand::Rng;
    let rng = rand::thread_rng();

    let sep = sep.into();

    let mut name = format!("tsunami{}{}{}", sep.0, prefix, sep.0);
    name.extend(rng.sample_iter(&rand::distributions::Alphanumeric).take(10));
    name
}

#[cfg(feature = "aws")]
pub mod aws;
#[cfg(feature = "azure")]
pub mod azure;
#[cfg(feature = "baremetal")]
pub mod baremetal;

fn setup_machine(
    log: &slog::Logger,
    nickname: &str,
    pub_ip: &str,
    username: &str,
    max_wait: Option<std::time::Duration>,
    private_key: Option<&std::path::Path>,
    f: &dyn Fn(&mut crate::ssh::Session, &slog::Logger) -> Result<(), Error>,
) -> Result<(), Error> {
    use failure::ResultExt;

    let mut m = crate::Machine {
        nickname: Default::default(),
        public_dns: pub_ip.to_string(),
        public_ip: pub_ip.to_string(),
        private_ip: None,
        ssh: None,
        _tsunami: Default::default(),
    };

    m.connect_ssh(log, username, private_key, max_wait)?;
    let mut sess = m.ssh.unwrap();

    debug!(log, "setting up instance"; "ip" => &pub_ip);
    f(&mut sess, log)
        .context(format!("setup procedure for {} machine failed", &nickname))
        .map_err(|e| {
            error!(
            log,
            "machine setup failed";
            "name" => &nickname,
            "ssh" => format!("ssh ubuntu@{}", &pub_ip),
            );
            e
        })?;
    info!(log, "finished setting up instance"; "name" => &nickname, "ip" => &pub_ip);
    Ok(())
}
