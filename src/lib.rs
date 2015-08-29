#![feature(convert, core)]
extern crate pyramid;
extern crate xml;

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::fs::File;
use std::io::BufReader;

use pyramid::interface::*;
use pyramid::propnode::*;
use pyramid::document::*;

use xml::reader::EventReader;
use xml::reader::Events;
use xml::reader::events::*;

#[derive(PartialEq, Debug, Clone)]
struct Template {
    type_name: String,
    inherits: Option<String>,
    properties: Vec<(String, PropNode)>,
    children: Vec<Template>
}

impl Template {
    fn from_string(string: &str) -> Result<Template, String> {
        let mut parser = EventReader::from_str(string);
        Template::from_event_reader(&mut parser.events())
    }
    fn from_event_reader<T: Iterator<Item=XmlEvent>>(mut event: &mut T) -> Result<Template, String> {
        let mut template_stack = vec![];
        while let Some(e) = event.next() {
            match e {
                XmlEvent::StartElement { name: type_name, attributes, .. } => {
                    let inherits = match attributes.iter().find(|x| x.name.local_name == "inherits") {
                        Some(attr) => Some(attr.value.to_string()),
                        None => None
                    };
                    let mut template = Template {
                        type_name: type_name.to_string(),
                        inherits: inherits,
                        properties: vec![],
                        children: vec![]
                    };
                    for attribute in attributes {
                        if (attribute.name.local_name == "inherits") { continue; }
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
                                None => return Ok(template)
                            };
                        }
                        None => return Err("Unbalanced xml tree".to_string())
                    }
                }
                XmlEvent::Error(e) => {
                    return Err(format!("Xml error: {}", e));
                }
                _ => {}
            }
        }
        Err("Unbalanced xml tree".to_string())
    }
    fn apply(&self, templates: &HashMap<String, Template>, system: &mut System, entity_id: &EntityId) {
        if let &Some(ref inherits) = &self.inherits {
            if let Some(inherits_template) = templates.get(inherits) {
                inherits_template.apply(templates, system, entity_id);
            }
        }
        for &(ref k, ref v) in &self.properties {
            system.set_property(entity_id, k.clone(), v.clone());
        }
        for ref template in &self.children {
            let e = system.append_entity(entity_id, template.type_name.clone(), None).unwrap();
            template.apply(templates, system, &e);
        }
    }
}

#[test]
fn test_template_from_string() {
    let str = r#"<Stone x="5"><Candle /></Stone>"#;
    let template = Template::from_string(str).unwrap();
    assert_eq!(template, Template {
        type_name: "Stone".to_string(),
        inherits: None,
        properties: vec![("x".to_string(), PropNode::Integer(5))],
        children: vec![
            Template {
                type_name: "Candle".to_string(),
                inherits: None,
                properties: vec![],
                children: vec![]
            }
        ]
    })
}

#[test]
fn test_template_apply() {
    let str = r#"<Stone x="5"><Candle /></Stone>"#;
    let template = Template::from_string(str).unwrap();
    let doc = Document::from_string(r#"<Stone name="tmp" />"#);
    let ent = doc.get_entity_by_name("tmp").unwrap();

    let mut system = pyramid::system::System::new();
    system.set_document(doc);
    template.apply(&HashMap::new(), &mut system, &ent);

    assert_eq!(system.get_property_value(&ent, "x"), Ok(PropNode::Integer(5)));
    assert_eq!(system.get_children(&ent).unwrap().len(), 1);
}

pub struct TemplateSubSystem {
    root_path: PathBuf,
    templates: HashMap<String, Template>
}

impl TemplateSubSystem {
    pub fn new(root_path: PathBuf) -> TemplateSubSystem {
        TemplateSubSystem {
            root_path: root_path,
            templates: HashMap::new()
        }
    }
    fn load_templates_from_file(&mut self, path: &Path) {
        let file = File::open(path).unwrap();
        let file = BufReader::new(file);

        let mut event_reader = EventReader::new(file);
        let mut events = event_reader.events().peekable();
        events.next(); // skip root
        while !events.is_empty() {
            match Template::from_event_reader(&mut events) {
                Ok(template) => { self.templates.insert(template.type_name.clone(), template); }
                _ => {}
            }
        }
    }
    fn load_templates(&mut self, node: &PropNode) -> Result<(), PropTranslateErr> {
        let templates = try!(node.as_array());
        for pn in templates {
            let p = try!(pn.as_transform());
            match p.name.as_str() {
                "template" => {
                    let s = try!(p.arg.as_string());
                    let template = Template::from_string(s).unwrap();
                    self.templates.insert(template.type_name.clone(), template);
                }
                "templates_from_file" => {
                    let filename = try!(p.arg.as_string());
                    let path = self.root_path.join(Path::new(filename));
                    self.load_templates_from_file(&path);
                }
                _ => return Err(PropTranslateErr::UnrecognizedPropTransform(p.name.clone()))
            }
        }
        Ok(())
    }
}


impl SubSystem for TemplateSubSystem {
    fn on_document_loaded(&mut self, system: &mut System) {
        let root = system.get_root().clone();
        let templates = system.get_property_value(&root, "templates").unwrap();
        self.load_templates(&templates);
        let entities: Vec<EntityId> = { system.get_entities().map(|x| x.clone()).collect() };
        for entity in entities {
            self.on_entity_added(system, &entity);
        }
    }
    fn on_entity_added(&mut self, system: &mut System, entity_id: &EntityId) {
        let type_name = system.get_entity_type_name(entity_id).unwrap().clone();
        match self.templates.get(&type_name) {
            Some(template) => {
                template.apply(&self.templates, system, entity_id);
            },
            None => {}
        }
    }
}

#[test]
fn test_template() {
    let template = r#"<Rock x="5"/>"#;
    let doc_src = format!(r#"<Root templates="[template '{}']"><Rock name="tmp" /></Root>"#, xml::escape::escape_str(template));
    let doc = Document::from_string(doc_src.as_str());
    let ent = doc.get_entity_by_name("tmp").unwrap();

    let mut system = pyramid::system::System::new();
    system.add_subsystem(Box::new(TemplateSubSystem::new(PathBuf::new())));
    system.set_document(doc);

    assert_eq!(system.get_property_value(&ent, "x"), Ok(PropNode::Integer(5)));
}

#[test]
fn test_template_inherits() {
    let template1 = r#"<Rock x="5"/>"#;
    let template2 = r#"<Granit inherits="Rock" y="2"/>"#;
    let doc_src = format!(r#"<Root templates="[template '{}', template '{}']"><Granit name="tmp" /></Root>"#, xml::escape::escape_str(template1), xml::escape::escape_str(template2));
    let doc = Document::from_string(doc_src.as_str());
    let ent = doc.get_entity_by_name("tmp").unwrap();

    let mut system = pyramid::system::System::new();
    system.add_subsystem(Box::new(TemplateSubSystem::new(PathBuf::new())));
    system.set_document(doc);

    assert_eq!(system.get_property_value(&ent, "x"), Ok(PropNode::Integer(5)));
    assert_eq!(system.get_property_value(&ent, "y"), Ok(PropNode::Integer(2)));
}
