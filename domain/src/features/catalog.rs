use schemars::SchemaGenerator;
use schemars::generate::SchemaSettings;
use serde_json::{Map, Value};

use super::ROUTES;
use crate::{AppError, Body};

#[derive(Debug)]
pub struct Entry {
    action: &'static str,
    body: Option<Value>,
    failures: &'static [AppError],
    feature: &'static str,
    method: &'static str,
    params: Option<Value>,
    path: &'static str,
    response: Value,
}

impl Entry {
    #[must_use = "动作名需写入接口说明"]
    pub const fn action(&self) -> &'static str {
        self.action
    }

    #[must_use = "请求体结构需写入接口说明"]
    pub const fn body(&self) -> Option<&Value> {
        self.body.as_ref()
    }

    #[must_use = "错误清单需写入接口说明"]
    pub const fn failures(&self) -> &'static [AppError] {
        self.failures
    }

    #[must_use = "功能名需写入接口说明"]
    pub const fn feature(&self) -> &'static str {
        self.feature
    }

    #[must_use = "方法需写入路由与接口说明"]
    pub const fn method(&self) -> &'static str {
        self.method
    }

    #[must_use = "参数结构需写入接口说明"]
    pub const fn params(&self) -> Option<&Value> {
        self.params.as_ref()
    }

    #[must_use = "路径需写入路由与接口说明"]
    pub const fn path(&self) -> &'static str {
        self.path
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
        .filter_map(|route| {
            let (method, path, describe) = route.http.binding()?;
            let (params, body, response) = describe(&mut generator);
            Some(Entry {
                action: route.action,
                body: body.map(Value::from),
                failures: route.failures,
                feature: route.feature,
                method,
                params: params.map(Value::from),
                path,
                response: response.into(),
            })
        })
        .collect();
    let error = generator.subschema_for::<Body<'static>>().into();
    Listing {
        entries,
        error,
        schemas: generator.take_definitions(true),
    }
}
