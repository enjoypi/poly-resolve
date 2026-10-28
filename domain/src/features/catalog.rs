use schemars::SchemaGenerator;
use schemars::generate::SchemaSettings;
use serde_json::{Map, Value};

use super::ROUTES;
use crate::Body;

#[derive(Debug)]
pub struct Entry {
    action: &'static str,
    feature: &'static str,
    request: Value,
    response: Value,
}

impl Entry {
    #[must_use = "动作名需写入接口说明"]
    pub const fn action(&self) -> &'static str {
        self.action
    }

    #[must_use = "功能名需写入接口说明"]
    pub const fn feature(&self) -> &'static str {
        self.feature
    }

    #[must_use = "请求结构需写入接口说明"]
    pub const fn request(&self) -> &Value {
        &self.request
    }

    #[must_use = "响应结构需写入接口说明"]
    pub const fn response(&self) -> &Value {
        &self.response
    }
}

#[derive(Debug)]
pub struct Listing {
    entries: Vec<Entry>,
    error: Value,
    schemas: Map<String, Value>,
}

impl Listing {
    #[must_use = "条目需写入接口说明"]
    pub const fn entries(&self) -> &[Entry] {
        self.entries.as_slice()
    }

    #[must_use = "错误体结构需写入接口说明"]
    pub const fn error(&self) -> &Value {
        &self.error
    }

    #[must_use = "共用结构需写入接口说明"]
    pub const fn schemas(&self) -> &Map<String, Value> {
        &self.schemas
    }
}

#[must_use = "接口清单需写入接口说明"]
pub fn build() -> Listing {
    let settings = SchemaSettings::draft2020_12().with(|settings| {
        settings.definitions_path = "/components/schemas".into();
        settings.meta_schema = None;
    });
    let mut generator = SchemaGenerator::new(settings);
    let entries = ROUTES
        .iter()
        .map(|route| {
            let (request, response) = (route.schema)(&mut generator);
            Entry {
                action: route.action,
                feature: route.feature,
                request: request.into(),
                response: response.into(),
            }
        })
        .collect();
    let error = generator.subschema_for::<Body<'static>>().into();
    Listing {
        entries,
        error,
        schemas: generator.take_definitions(true),
    }
}
