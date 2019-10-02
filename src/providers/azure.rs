use crate::ssh;
use failure::{bail, Error, ResultExt};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Region {
    EastUs,
    EastUs2,
    WestUs,
    CentralUs,
    NorthCentralUs,
    SouthCentralUs,
    NorthEurope,
    WestEurope,
    EastUsia,
    SouthEastUsia,
    JapanEast,
    JapanWest,
    AustraliaEast,
    AustraliaSoutheast,
    AustraliaCentral,
    BrazilSouth,
    SouthIndia,
    CentralIndia,
    WestIndia,
    CanadaCentral,
    CanadaEast,
    WestUs2,
    WestCentralus,
    UkSouth,
    UkWest,
    KoreaCentral,
    KoreaSouth,
    FranceCentral,
    SouthAfricaNorth,
    UaeNorth,
    GermanyWestCentral,
}

impl Default for Region {
    fn default() -> Self {
        Region::EastUs
    }
}

impl std::string::ToString for Region {
    fn to_string(&self) -> String {
        String::from(match self {
            Region::EastUs => "eastus",
            Region::EastUs2 => "eastus2",
            Region::WestUs => "westus",
            Region::CentralUs => "centralus",
            Region::NorthCentralUs => "northcentralus",
            Region::SouthCentralUs => "southcentralus",
            Region::NorthEurope => "northeurope",
            Region::WestEurope => "westeurope",
            Region::EastUsia => "eastasia",
            Region::SouthEastUsia => "southeastasia",
            Region::JapanEast => "japaneast",
            Region::JapanWest => "japanwest",
            Region::AustraliaEast => "australiaeast",
            Region::AustraliaSoutheast => "australiasoutheast",
            Region::AustraliaCentral => "australiacentral",
            Region::BrazilSouth => "brazilsouth",
            Region::SouthIndia => "southindia",
            Region::CentralIndia => "centralindia",
            Region::WestIndia => "westindia",
            Region::CanadaCentral => "canadacentral",
            Region::CanadaEast => "canadaeast",
            Region::WestUs2 => "westus2",
            Region::WestCentralus => "westcentralus",
            Region::UkSouth => "uksouth",
            Region::UkWest => "ukwest",
            Region::KoreaCentral => "koreacentral",
            Region::KoreaSouth => "koreasouth",
            Region::FranceCentral => "francecentral",
            Region::SouthAfricaNorth => "southafricanorth",
            Region::UaeNorth => "uaenorth",
            Region::GermanyWestCentral => "germanywestcentral",
        })
    }
}

impl std::str::FromStr for Region {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "eastus" => Region::EastUs,
            "eastus2" => Region::EastUs2,
            "westus" => Region::WestUs,
            "centralus" => Region::CentralUs,
            "northcentralus" => Region::NorthCentralUs,
            "southcentralus" => Region::SouthCentralUs,
            "northeurope" => Region::NorthEurope,
            "westeurope" => Region::WestEurope,
            "eastasia" => Region::EastUsia,
            "southeastasia" => Region::SouthEastUsia,
            "japaneast" => Region::JapanEast,
            "japanwest" => Region::JapanWest,
            "australiaeast" => Region::AustraliaEast,
            "australiasoutheast" => Region::AustraliaSoutheast,
            "australiacentral" => Region::AustraliaCentral,
            "brazilsouth" => Region::BrazilSouth,
            "southindia" => Region::SouthIndia,
            "centralindia" => Region::CentralIndia,
            "westindia" => Region::WestIndia,
            "canadacentral" => Region::CanadaCentral,
            "canadaeast" => Region::CanadaEast,
            "westus2" => Region::WestUs2,
            "westcentralus" => Region::WestCentralus,
            "uksouth" => Region::UkSouth,
            "ukwest" => Region::UkWest,
            "koreacentral" => Region::KoreaCentral,
            "koreasouth" => Region::KoreaSouth,
            "francecentral" => Region::FranceCentral,
            "southafricanorth" => Region::SouthAfricaNorth,
            "uaenorth" => Region::UaeNorth,
            "germanywestcentral" => Region::GermanyWestCentral,
            u => bail!("Unknown azure region {}", u),
        })
    }
}

mod azcmd {
    use super::Error;
    use super::Region;
    use failure::ResultExt;
    use serde::{Deserialize, Serialize};
    use std::process::Command;

    pub fn available_instances_in_region(r: Region) -> Result<Vec<String>, Error> {
        #[allow(non_snake_case)]
        #[derive(Debug, Deserialize, Serialize)]
        struct VmSizeDescriptor {
            maxDataDiskCount: usize,
            memoryInMb: usize,
            name: String,
            numberOfCores: usize,
            osDiskSizeInMb: usize,
            resourceDiskSizeInMb: usize,
        }

        let out = Command::new("az")
            .args(&["vm", "list-sizes", "-l", &r.to_string()])
            .output()?;
        if !out.status.success() {
            bail!("Failed to get available instance sizes");
        }

        let v: Vec<VmSizeDescriptor> = serde_json::from_slice(&out.stdout)?;
        Ok(v.into_iter().map(|x| x.name).collect())
    }

    pub fn create_resource_group(r: Region, name: &str) -> Result<(), Error> {
        let out = Command::new("az")
            .args(&[
                "group",
                "create",
                "--name",
                name,
                "--location",
                &r.to_string(),
            ])
            .output()?;

        if !out.status.success() {
            bail!("Failed to create resource group {} in region {:?}", name, r)
        }

        Ok(())
    }

    pub fn create_vm(rg: &str, name: &str, size: &str, username: &str) -> Result<String, Error> {
        #[allow(non_snake_case)]
        #[derive(Debug, Deserialize, Serialize)]
        struct VmCreateOut {
            powerState: String,
            publicIpAddress: String,
            resourceGroup: String,
        }

        let out = Command::new("az")
            .args(&[
                "vm",
                "create",
                "--resource-group",
                rg,
                "--name",
                name,
                "--image",
                "UbuntuLTS",
                "--size",
                size,
                "--admin-username",
                username,
                "--ssh-key-value",
                "@~/.ssh/id_rsa.pub",
            ])
            .output()?;

        if !out.status.success() {
            return Err(format_err!("Failed to create vm {}", name))
                .context(String::from_utf8(out.stderr).unwrap())?;
        }

        let vm: VmCreateOut = serde_json::from_slice(&out.stdout)?;
        ensure!(vm.powerState == "VM running", "VM power state incorrect");
        ensure!(vm.resourceGroup == rg, "VM resource group incorrect");
        Ok(vm.publicIpAddress)
    }

    pub fn open_ports(rg: &str, vm_name: &str) -> Result<(), Error> {
        let out = Command::new("az")
            .args(&[
                "vm",
                "open-port",
                "--port",
                "0-65535",
                "--resource-group",
                rg,
                "--name",
                vm_name,
            ])
            .output()?;
        if !out.status.success() {
            return Err(format_err!("Failed to open ports for {}", vm_name))
                .context(String::from_utf8(out.stderr).unwrap())?;
        }

        Ok(())
    }

    pub fn delete_resource_group(rg: &str) -> Result<(), Error> {
        let out = Command::new("az")
            .args(&["group", "delete", "--name", rg, "--yes"])
            .output()?;
        if !out.status.success() {
            bail!("Failed to delete resource group {}", rg)
        }

        Ok(())
    }
}

/// A descriptor for a single Azure VM type. Only UbuntuLTS VMs are supported.
pub struct Setup {
    region: Region,
    instance_type: String,
    setup_fn:
        Option<Box<dyn Fn(&mut ssh::Session, &slog::Logger) -> Result<(), Error> + Send + Sync>>,
}

impl Default for Setup {
    fn default() -> Self {
        Setup {
            region: "eastus".parse().unwrap(),
            instance_type: "Standard_DS1_v2".to_string(),
            setup_fn: None,
        }
    }
}

impl super::MachineSetup for Setup {
    type Region = Region;

    fn region(&self) -> Self::Region {
        self.region
    }
}

impl Setup {
    pub fn region(self, r: Region) -> Self {
        Self { region: r, ..self }
    }

    /// To view the available sizes in the relevant region, use:
    /// `az vm list-sizes -l <region_name>`.
    pub fn instance_type(self, inst_type: String) -> Result<Self, Error> {
        if azcmd::available_instances_in_region(self.region)?
            .iter()
            .any(|x| x == &inst_type)
        {
            Ok(Self {
                instance_type: inst_type,
                ..self
            })
        } else {
            Err(format_err!(
                "{} not valid instance type in {:?}",
                inst_type,
                self.region
            ))
        }
    }

    /// The `setup` argument is called once for every spawned instances of this type with a handle
    /// to the target machine. Use [`Machine::ssh`](struct.Machine.html#structfield.ssh) to issue
    /// commands on the host in question.
    pub fn setup(
        mut self,
        setup: impl Fn(&mut ssh::Session, &slog::Logger) -> Result<(), Error> + Send + Sync + 'static,
    ) -> Self {
        self.setup_fn = Some(Box::new(setup));
        self
    }
}

/// This implementation relies on the [Azure
/// CLI](https://docs.microsoft.com/en-us/cli/azure/install-azure-cli?view=azure-cli-latest). 
///
/// It also assumes you have previously run `az login` to authenticate with Microsoft.
#[derive(Default)]
pub struct AzureLauncher {
    regions: HashMap<Region, AzureRegion>,
}

impl super::Launcher for AzureLauncher {
    type Machine = Setup;

    fn launch(&mut self, l: super::LaunchDescriptor<Self::Machine>) -> Result<(), Error> {
        let region = l.region;
        let mut az_region = AzureRegion::new(&l.region.to_string(), l.log.clone())?;
        az_region.launch(l)?;
        self.regions.insert(region, az_region);
        Ok(())
    }

    fn connect_all<'l>(&'l self) -> Result<HashMap<String, crate::Machine<'l>>, Error> {
        collect!(self.regions)
    }
}

impl std::ops::Drop for AzureLauncher {
    fn drop(&mut self) { }
}

struct Descriptor {
    name: String,
    ip: String,
}

/// Region-specific connection to Azure. Each instance of this type creates one Azure
/// "resource group" and deletes the group on drop.
///
/// This implementation relies on the [Azure
/// CLI](https://docs.microsoft.com/en-us/cli/azure/install-azure-cli?view=azure-cli-latest). 
///
/// It also assumes you have previously run `az login` to authenticate with Microsoft.
#[derive(Default)]
pub struct AzureRegion {
    pub log: Option<slog::Logger>,
    pub region: Region,
    resource_group_name: String,
    machines: Vec<Descriptor>,
}

impl AzureRegion {
    pub fn new(region: &str, log: slog::Logger) -> Result<Self, Error> {
        let region = region.parse()?;
        let rg_name = super::rand_name("resourcegroup");

        azcmd::create_resource_group(region, &rg_name)?;

        Ok(Self {
            log: Some(log),
            region,
            resource_group_name: rg_name,
            machines: vec![],
        })
    }
}

impl super::Launcher for AzureRegion {
    type Machine = Setup;

    fn launch(&mut self, l: super::LaunchDescriptor<Self::Machine>) -> Result<(), Error> {
        self.log = Some(l.log);
        let log = self.log.as_ref().unwrap();
        let max_wait = l.max_wait;
        self.machines = 
        l.machines
            .into_iter()
            .map(|(nickname, desc)| {
                let vm_name = super::rand_name_sep("vm", "-");
                debug!(log, "setting up azure instance"; "nickname" => &nickname, "vm_name" => &vm_name);

                let pub_ip = azcmd::create_vm(
                    &self.resource_group_name,
                    &vm_name,
                    &desc.instance_type,
                    "ubuntu",
                )?;

                azcmd::open_ports(&self.resource_group_name, &vm_name)?;

                let mut sess = ssh::Session::connect(
                    log,
                    "ubuntu",
                    SocketAddr::new(
                        pub_ip
                            .clone()
                            .parse::<IpAddr>()
                            .context("machine ip is not an ip address")?,
                        22,
                    ),
                    None,
                    max_wait,
                )
                .context(format!("failed to ssh to machine {}", nickname.clone()))
                .map_err(|e| {
                    error!(log, "failed to ssh to {}", &pub_ip.clone());
                    e
                })?;


                match desc {
                    Setup { setup_fn: Some(f), .. } => {
                        debug!(log, "setting up instance"; "ip" => &pub_ip);
                        f(&mut sess, log)
                            .context(format!(
                                "setup procedure for {} machine failed",
                                &nickname
                            ))
                            .map_err(|e| {
                                error!(
                                    log,
                                    "machine setup failed";
                                    "name" => &nickname,
                                    "ssh" => format!("ssh ubuntu@{}", &pub_ip),
                                );
                                e
                            })?;
                    }
                    _ => {},
                }

                info!(log, "finished setting up instance"; "name" => &nickname, "ip" => &pub_ip);
                Ok(Descriptor { name: nickname, ip: pub_ip })
            })
            .collect::<Result<Vec<Descriptor>, Error>>()?;
        Ok(())
    }

    fn connect_all<'l>(&'l self) -> Result<HashMap<String, crate::Machine<'l>>, Error> {
        let log = self.log.as_ref().expect("AzureRegion uninitialized");
        self.machines
            .iter()
            .map(|desc| {
                let Descriptor { name, ip } = desc;
                let sess = ssh::Session::connect(
                    log,
                    "ubuntu",
                    SocketAddr::new(
                        ip.clone()
                            .parse::<IpAddr>()
                            .context("machine ip is not an ip address")?,
                        22,
                    ),
                    None,
                    None,
                )
                .context(format!("failed to ssh to machine {}", name.clone()))
                .map_err(|e| {
                    error!(log, "failed to ssh to {}", &ip.clone());
                    e
                })?;
                use crate::Machine;
                Ok((
                    name.clone(),
                    Machine {
                        nickname: name.clone(),
                        public_dns: ip.clone(),
                        public_ip: ip.clone(),
                        ssh: Some(sess),
                        _tsunami: Default::default(),
                    },
                ))
            })
            .collect()
    }
}

impl Drop for AzureRegion {
    fn drop(&mut self) {
        azcmd::delete_resource_group(&self.resource_group_name).unwrap();
    }
}

#[cfg(test)]
mod test {
    use super::azcmd;
    use super::Region;

    #[test]
    #[ignore]
    fn resource_group() {
        static TEST_RG_NAME: &str = "test";
        azcmd::create_resource_group(Region::EastUs, TEST_RG_NAME)
            .expect("create resource group test failed");

        azcmd::delete_resource_group(TEST_RG_NAME).expect("delete resource group failed");
    }
}
