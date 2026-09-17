use anyhow::{Context, Result};
use uuid::Uuid;

use crate::{
    libvirt::libvirt_domain::{Channel, Console, Disk, Domain, Interface, Mac},
    model::VmConfig,
    paths::InstancePaths,
};

pub mod libvirt_domain;

pub fn generate_domain(config: &VmConfig, paths: &InstancePaths) -> Result<(String, Uuid)> {
    let hostname = config.instance.hostname.clone();
    let uuid = Uuid::new_v4();
    let mut domain = Domain::new(
        hostname.clone(),
        config.instance.memory,
        config.instance.vcpus,
    );
    domain.uuid = Some(uuid.into());
    domain.devices.disks = vec![
        Disk::file(
            paths
                .disk
                .to_str()
                .context("disk path is not valid UTF-8 for libvirt XML")?,
            "sdb",
            "qcow2",
        ),
        Disk::cdrom(
            paths
                .seed_iso
                .to_str()
                .context("seed ISO path is not valid UTF-8 for libvirt XML")?,
            "sda",
        ),
    ];
    let mut interface = Interface::network("andromeda-users");
    interface.mac = Some(Mac {
        address: config.networking.mac.clone(),
    });
    domain.devices.interfaces.push(interface);
    domain.devices.consoles.push(Console::serial());
    domain.devices.channels.push(Channel::guest_agent());

    Ok((domain.to_xml()?, uuid))
}
