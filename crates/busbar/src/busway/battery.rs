use uuid::Uuid;

pub struct Meta {
    pub symbol: String,
    pub begins_at: i64,
    pub ends_at: i64,
    pub frequency: String,
}

pub struct Battery {
    pub uid: String,
    data: polars::prelude::DataFrame,
    meta: Meta,
}

impl Battery {
    pub fn new(symbol: String, source: super::Source) -> Self {
        let uid = Uuid::new_v4().to_string();
        let meta = Meta {
            symbol,
            begins_at: 0,
            ends_at: 0,
            frequency: "1d".to_string(),
        };
        let data = polars::prelude::DataFrame::default();
        Self { uid, data, meta }
    }

    pub fn get_data(&self, filters: &Vec<super::Filters>) -> polars::prelude::DataFrame {
        if self.data_meets_filters(filters) {
            // TODO: apply filters to data
            self.data.clone()
        } else {
            // TODO: from data source pull needed data
            polars::prelude::DataFrame::default()
        }
    }

    fn data_meets_filters(&self, filters: &Vec<super::Filters>) -> bool {
        // check if the data meets the filters
        for filter in filters {
            match filter {
                super::Filters::DateRange { start, end } => {
                    if self.meta.begins_at > *start || self.meta.ends_at < *end {
                        return false;
                    }
                }
                super::Filters::Frequency(freq) => {
                    if &self.meta.frequency != freq {
                        return false;
                    }
                }
            }
        }
        true
    }
}
