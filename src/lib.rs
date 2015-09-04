#![feature(convert, core)]
extern crate pyramid;
extern crate xml;

mod template;

use template::*;

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::fs::File;
use std::io::BufReader;

use pyramid::interface::*;
use pyramid::pon::*;
use pyramid::document::*;

use xml::reader::EventReader;
use xml::reader::Events;
use xml::reader::events::*;

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
        let mut events = event_reader.events();
        let mut template_stack = vec![];
        while let Some(e) = events.next() {
            match e.clone() {
                XmlEvent::StartElement { name, .. } => {
                    if name.local_name.as_str() == "Tpml" {
                        continue;
                    }
                }
                XmlEvent::EndElement { name, .. } => {
                    if name.local_name.as_str() == "Tpml" {
                        continue;
                    }
                }
                _ => {}
            }
            match Template::parse_event(&mut template_stack, e) {
                Some(template) => { self.templates.insert(template.type_name.clone(), template); }
                _ => {}
            }
        }
    }
    fn load_templates(&mut self, node: &Pon) -> Result<(), PonTranslateErr> {
        let templates = try!(node.as_array());
        for pn in templates {
            let p = try!(pn.as_transform());
            match p.type_name.as_str() {
                "template" => {
                    let s = try!(p.data.as_string());
                    let template = Template::from_string(s).unwrap();
                    self.templates.insert(template.type_name.clone(), template);
                }
                "templates_from_file" => {
                    let filename = try!(p.data.as_string());
                    let path = self.root_path.join(Path::new(filename));
                    self.load_templates_from_file(&path);
                }
                _ => return Err(PonTranslateErr::UnrecognizedType(p.type_name.clone()))
            }
        }
        Ok(())
    }
}


impl ISubSystem for TemplateSubSystem {
    fn on_document_loaded(&mut self, system: &mut ISystem) {
        let root = system.get_root().clone();
        match system.get_property_value(&root, "templates") {
            Ok(templates) => {
                self.load_templates(&templates);
                let entities: Vec<EntityId> = { system.get_entities().map(|x| x.clone()).collect() };
                for entity in entities {
                    self.on_entity_added(system, &entity);
                }
                println!("TEMPLATES {:?}", self.templates);
            },
            _ => {}
        }
    }
    fn on_entity_added(&mut self, system: &mut ISystem, entity_id: &EntityId) {
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

    assert_eq!(system.get_property_value(&ent, "x"), Ok(Pon::Integer(5)));
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

    assert_eq!(system.get_property_value(&ent, "x"), Ok(Pon::Integer(5)));
    assert_eq!(system.get_property_value(&ent, "y"), Ok(Pon::Integer(2)));
}
