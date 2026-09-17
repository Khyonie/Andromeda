use std::{
    collections::BTreeMap,
    io::Write,
    process::{Command, Stdio},
};

use quick_xml::{Reader, events::Event};

use super::*;

const REFERENCE: &str = include_str!("../../../domain_reference.xml");

fn reference_domain() -> Domain {
    let mut domain = Domain::new("vm-alice", 512, 1);
    // The reference fixture predates the ACPI/APIC defaults used for new guests.
    domain.features = Features::default();
    domain.uuid = Some("0d45f535-cfeb-48de-800d-2b0ed8bb0bca".into());
    domain.devices.disks = vec![
        Disk::file("/var/lib/libvirt/images/vms/vm-alice.qcow2", "vda", "qcow2"),
        Disk::cdrom("/var/lib/libvirt/images/seed/vm-alice.iso", "sda"),
    ];
    let mut interface = Interface::network("andromeda-users");
    interface.mac = Some(Mac {
        address: "52:54:00:10:00:01".into(),
    });
    domain.devices.interfaces.push(interface);
    domain.devices.consoles.push(Console::serial());
    domain.devices.channels.push(Channel::guest_agent());
    domain
}

/// Compare every XML node and attribute independently of Serde, ignoring only
/// formatting, attribute order, and the spelling of empty elements.
fn xml_events(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    reader.config_mut().expand_empty_elements = true;
    let mut events = Vec::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(start) => {
                let attrs: BTreeMap<_, _> = start
                    .attributes()
                    .map(|attr| {
                        let attr = attr.unwrap();
                        (
                            attr.key.as_ref().to_owned(),
                            attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .unwrap()
                                .into_owned(),
                        )
                    })
                    .collect();
                events.push(format!("start:{}:{attrs:?}", start.name().as_ref()));
            }
            Event::End(end) => events.push(format!("end:{}", end.name().as_ref())),
            Event::Text(text) => events.push(format!(
                "text:{}",
                quick_xml::escape::unescape(text.as_ref()).unwrap()
            )),
            Event::Eof => break,
            Event::Comment(_) | Event::Decl(_) => {}
            other => panic!("Unexpected XML event: {other:?}"),
        }
    }
    events
}

#[test]
fn constructed_domain_matches_every_reference_element_and_attribute() {
    let domain = reference_domain();
    let xml = domain.to_xml().unwrap();
    assert_eq!(xml_events(&xml), xml_events(REFERENCE));
    assert_eq!(quick_xml::se::to_string(&domain).unwrap(), xml);
}

#[test]
fn reference_xml_round_trips_without_losing_content() {
    let domain: Domain = quick_xml::de::from_str(REFERENCE).unwrap();
    assert_eq!(domain, reference_domain());
    assert_eq!(xml_events(&domain.to_xml().unwrap()), xml_events(REFERENCE));
}

#[test]
fn absent_fields_and_empty_device_lists_are_omitted() {
    let mut domain = Domain::new("minimal", 512, 1);
    domain.features = Features::default();
    let xml = domain.to_xml().unwrap();
    assert!(!xml.contains("<uuid"));
    assert!(!xml.contains("<features"));
    assert!(xml.contains("<devices/>"));
    assert_eq!(quick_xml::de::from_str::<Domain>(&xml).unwrap(), domain);
    let interface = Interface::network("andromeda-users");
    assert!(
        !quick_xml::se::to_string(&interface)
            .unwrap()
            .contains("<mac")
    );
}

#[test]
fn optional_guest_features_round_trip() {
    for (features_xml, acpi, apic) in [
        ("<features><acpi/><apic/></features>", true, true),
        ("<features><acpi/></features>", true, false),
        ("<features><apic/></features>", false, true),
        ("<features/>", false, false),
    ] {
        let features = Features {
            acpi: acpi.then_some(Empty {}),
            apic: apic.then_some(Empty {}),
        };
        assert_eq!(
            quick_xml::de::from_str::<Features>(features_xml).unwrap(),
            features
        );
        let mut domain = Domain::new("features", 512, 1);
        let empty = features.is_empty();
        domain.features = features;
        let xml = domain.to_xml().unwrap();
        if empty {
            assert!(!xml.contains("<features"), "{xml}");
        } else {
            assert!(xml.contains(features_xml), "{xml}");
        }
        assert_eq!(quick_xml::de::from_str::<Domain>(&xml).unwrap(), domain);
    }
}

#[test]
fn text_and_attributes_are_escaped() {
    let mut domain = reference_domain();
    domain.name = "vm<&>".into();
    domain.devices.disks[0].source.file = "/images/a&b\"<disk>.qcow2".into();
    let xml = domain.to_xml().unwrap();
    assert!(xml.contains("<name>vm&lt;&amp;&gt;</name>"));
    assert!(xml.contains("file=\"/images/a&amp;b&quot;&lt;disk&gt;.qcow2\""));
    assert_eq!(quick_xml::de::from_str::<Domain>(&xml).unwrap(), domain);
}

/// Optional external validation; no daemon, image files, or VM is needed.
#[test]
#[ignore = "requires xmllint and libvirt's domain.rng; see domain/README.md"]
fn generated_reference_validates_against_libvirt_schema() {
    let schema = std::env::var("LIBVIRT_DOMAIN_SCHEMA")
        .unwrap_or_else(|_| "/usr/share/libvirt/schemas/domain.rng".into());
    let xml = reference_domain().to_xml().unwrap();
    let mut child = Command::new("xmllint")
        .args(["--nonet", "--noout", "--relaxng", &schema, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("xmllint must be installed");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(xml.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
