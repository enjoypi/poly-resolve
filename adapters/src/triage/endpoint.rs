use core::net::IpAddr;
use core::num::NonZeroU64;

use http::Uri;
use serde::Deserialize;

use super::config::Var;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Http {
    key: Option<Var>,
    timeout_ms: NonZeroU64,
    url: Location,
}

impl Http {
    pub(crate) fn key(&self) -> Option<String> {
        self.key.as_ref().and_then(Var::value)
    }

    pub(crate) fn local_url(&self) -> Result<Uri, String> {
        let url = self.parsed()?;
        if url.scheme_str() == Some("http") && url.port().is_some() && loopback(&url) {
            Ok(url)
        } else {
            Err("loopback http url required".to_owned())
        }
    }

    fn parsed(&self) -> Result<Uri, String> {
        let url = self
            .url
            .value()
            .unwrap_or_default()
            .parse::<Uri>()
            .map_err(|_err| "invalid url".to_owned())?;
        if url.host().is_some()
            && url.scheme_str().is_some()
            && url
                .authority()
                .is_none_or(|authority| !authority.as_str().contains('@'))
        {
            Ok(url)
        } else {
            Err("url credentials or host invalid".to_owned())
        }
    }

    pub(crate) const fn timeout_ms(&self) -> u64 {
        self.timeout_ms.get()
    }

    pub(crate) fn url(&self) -> Result<Uri, String> {
        let url = self.parsed()?;
        if url.scheme_str() == Some("https") && self.url.var().is_none() {
            return Err("external url must use ${NAME}".to_owned());
        }
        if url.scheme_str() == Some("https") || (url.scheme_str() == Some("http") && loopback(&url))
        {
            Ok(url)
        } else {
            Err("https or loopback http url required".to_owned())
        }
    }

    pub(crate) fn vars(&self) -> impl Iterator<Item = &Var> {
        self.url.var().into_iter().chain(self.key.iter())
    }
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "String")]
struct Location {
    source: String,
    var: Option<Var>,
}

impl Location {
    fn value(&self) -> Option<String> {
        self.var
            .as_ref()
            .and_then(Var::value)
            .or_else(|| Some(self.source.clone()))
    }

    const fn var(&self) -> Option<&Var> {
        self.var.as_ref()
    }
}

impl TryFrom<String> for Location {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let var = Var::try_from(value.clone()).ok();
        Ok(Self { source: value, var })
    }
}

fn loopback(url: &Uri) -> bool {
    url.host().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    })
}
