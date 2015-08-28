#![feature(convert)]
extern crate pyramid;
extern crate xml;

use std::collections::HashMap;

use pyramid::interface::*;
use pyramid::propnode::*;
use pyramid::document::*;

use xml::reader::EventReader;
use xml::reader::Events;
use xml::reader::events::*;

#[derive(PartialEq, Debug, Clone)]
struct Template {
    type_name: String,
    properties: Vec<(String, PropNode)>,
    children: Vec<Template>
}

impl Template {
    fn from_string(string: &str) -> Template {
        let mut parser = EventReader::from_str(string);
        Template::from_event_reader(parser.events())
    }
    fn from_event_reader<T: Iterator<Item=XmlEvent>>(mut event: T) -> Template {
        let mut template_stack = vec![];
        while let Some(e) = event.next() {
            match e {
                XmlEvent::StartElement { name: type_name, attributes, .. } => {

                    let mut template = Template {
                        type_name: type_name.to_string(),
                        properties: vec![],
                        children: vec![]
                    };
                    for attribute in attributes {
                        match pyramid::propnode_parser::parse(&attribute.value) {
                            Ok(node) => template.properties.push((attribute.name.local_name.to_string(), node)),
                            Err(err) => panic!("Error parsing: {} error: {:?}", attribute.value, err)
                        };
                    }
                    template_stack.push(template);
                }
                XmlEvent::EndElement { name: type_name } => {
                    match template_stack.pop() {
                        Some(template) => {
                            match template_stack.last_mut() {
                                Some(ref mut parent) => {
                                    parent.children.push(template);
                                }
                                None => return template
                            };
                        }
                        None => panic!("Unbalanced template xml")
                    }
                }
                XmlEvent::Error(e) => {
                    println!("Error: {}", e);
                    break;
                }
                _ => {}
            }
        }
        panic!("Unbalanced template xml");
    }
    fn apply(&self, system: &mut System, entity_id: &EntityId) {
        for &(ref k, ref v) in &self.properties {
            system.set_property(entity_id, k.clone(), v.clone());
        }
        for ref template in &self.children {
            let e = system.append_entity(entity_id, template.type_name.clone(), None).unwrap();
            template.apply(system, &e);
        }
    }
}

#[test]
fn test_template_from_string() {
    let str = r#"<Stone x="5"><Candle /></Stone>"#;
    let template = Template::from_string(str);
    assert_eq!(template, Template {
        type_name: "Stone".to_string(),
        properties: vec![("x".to_string(), PropNode::Integer(5))],
        children: vec![
            Template {
                type_name: "Candle".to_string(),
                properties: vec![],
                children: vec![]
            }
        ]
    })
}

#[test]
fn test_template_apply() {
    let str = r#"<Stone x="5"><Candle /></Stone>"#;
    let template = Template::from_string(str);
    let doc = Document::from_string(r#"<Stone name="tmp" />"#);
    let ent = doc.get_entity_by_name("tmp").unwrap();

    let mut system = pyramid::system::System::new();
    system.set_document(doc);
    template.apply(&mut system, &ent);

    assert_eq!(system.get_property_value(&ent, "x"), Ok(PropNode::Integer(5)));
    assert_eq!(system.get_children(&ent).unwrap().len(), 1);
}
//
// pub struct TemplateSubSystem {
//     templates: HashMap<String, Template>
// }
//
// impl TemplateSubSystem {
//     pub fn new() -> TemplateSubSystem {
//         TemplateSubSystem {
//             templates: HashMap::new()
//         }
//     }
//     fn load_templates(&mut self, node: &PropNode) -> Result<(), PropTranslateErr> {
//         let p = try!(node.as_transform());
//         if p.name != "templates" {
//             return Err(PropTranslateErr::UnrecognizedPropTransform(p.name));
//         }
//         let templates = try!(p.arg.as_array());
//         for pn in templates {
//             let s = try!(pn.as_string());
//             let template = Template::from_string(s);
//             self.templates.insert(template.type_name, tempalte);
//         }
//     }
//
// }
//
//
// impl SubSystem for TemplateSubSystem {
//     fn on_document_loaded(&mut self, system: &mut System) {
//         let root = system.get_root();
//         let templates = system.get_property_value(root, "templates");
//         self.load_templates(templates);
//         for entity in system.get_entities() {
//             self.on_entity_added(system, entity);
//         }
//     }
//     fn on_entity_added(&mut self, system: &mut System, entity_id: &EntityId) {
//         let type_name = system.get_entity_type_name(entity_id);
//         match self.templates.get(type_name) {
//             Some(tempalte) => {
//                 self.apply_template(system, entity_id, template);
//             },
//             None => {}
//         }
//     }
//     fn on_property_value_change(&mut self, system: &mut System, prop_refs: &Vec<PropRef>) {
//     }
//     fn update(&mut self, system: &mut System, delta_time: time::Duration) {
//     }
// }
//
// #[test]
// fn test_template() {
//     let doc = Document::from_string(r#"<Root templates="templates ['<Rock x=\"5\">']"><Rock name="tmp" /></Root>"#);
//     let ent = doc.get_entity_by_name("tmp").unwrap();
//
//     let mut system = System::new();
//     system.add_subsystem(Box::new(TemplateSubSystem::new()));
//     system.set_document(doc);
//
//     assert_eq!(system.get_property_value(&ent, "x"), Ok(PropNode::Integer(5)));
// }
