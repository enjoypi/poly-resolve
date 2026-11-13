use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Question {
    name: String,
}

impl Question {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Questions {
    review: Review,
    threshold: f64,
    topic: Topic,
    urgent: Question,
}

impl Questions {
    pub(crate) const fn review(&self) -> &Review {
        &self.review
    }

    pub(crate) const fn threshold(&self) -> f64 {
        self.threshold
    }

    pub(crate) const fn topic(&self) -> &Topic {
        &self.topic
    }

    pub(crate) const fn urgent(&self) -> &Question {
        &self.urgent
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Review {
    name: String,
    threshold: f64,
}

impl Review {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) const fn threshold(&self) -> f64 {
        self.threshold
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Topic {
    min_confidence: f64,
    name: String,
    new_label: String,
    none_label: String,
}

impl Topic {
    pub(crate) const fn min_confidence(&self) -> f64 {
        self.min_confidence
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn new_label(&self) -> &str {
        &self.new_label
    }

    pub(crate) fn none_label(&self) -> &str {
        &self.none_label
    }
}
