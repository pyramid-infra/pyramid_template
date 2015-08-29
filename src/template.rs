extern crate pyramid;

use std::collections::HashMap;

use pyramid::propnode::*;
use pyramid::interface::*;
use pyramid::document::*;

use xml::reader::EventReader;
use xml::reader::Events;
use xml::reader::events::*;

#[derive(PartialEq, Debug, Clone)]
pub struct Template {
    pub type_name: String,
    pub inherits: Option<String>,
    pub properties: Vec<(String, PropNode)>,
    pub children: Vec<Template>
}

impl Template {
    pub fn from_string(string: &str) -> Result<Template, String> {
        let mut parser = EventReader::from_str(string);
        let mut event = parser.events();
        let mut template_stack = vec![];
        while let Some(e) = event.next() {
            match Template::parse_event(&mut template_stack, e) {
                Some(template) => return Ok(template),
                None => {}
            }
        }
        Err("No template parsed".to_string())
    }
    pub fn parse_event(mut template_stack: &mut Vec<Template>, event: XmlEvent) -> Option<Template> {
        match event {
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
                            None => return Some(template)
                        };
                    }
                    None => return None
                }
            }
            XmlEvent::Error(e) => {
                panic!("Xml error: {}", e);
            }
            _ => {}
        }
        None
    }
    pub fn apply(&self, templates: &HashMap<String, Template>, system: &mut System, entity_id: &EntityId) {
        if let &Some(ref inherits) = &self.inherits {
            if let Some(inherits_template) = templates.get(inherits) {
                inherits_template.apply(templates, system, entity_id);
            }
        }
        for &(ref k, ref v) in &self.properties {
            if let Ok(has) = system.has_property(entity_id, &k.as_str()) {
                if !has {
                    system.set_property(entity_id, k.clone(), v.clone());
                }
            }
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

#[test]
fn test_template_apply_dont_overwrite() {
    let str = r#"<Stone x="5" />"#;
    let template = Template::from_string(str).unwrap();
    let doc = Document::from_string(r#"<Stone x="7" name="tmp" />"#);
    let ent = doc.get_entity_by_name("tmp").unwrap();

    let mut system = pyramid::system::System::new();
    system.set_document(doc);
    template.apply(&HashMap::new(), &mut system, &ent);

    assert_eq!(system.get_property_value(&ent, "x"), Ok(PropNode::Integer(7)));
}
