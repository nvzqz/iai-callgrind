use anyhow::Result;
use log::{debug, trace};

use super::parser::{Parser, Sentinel};
use super::{CallgrindOutput, CallgrindStats};
use crate::error::Error;
use crate::runner::callgrind::parser::parse_header;

pub struct SentinelParser {
    sentinel: Sentinel,
}

impl SentinelParser {
    pub fn new(sentinel: &Sentinel) -> Self {
        Self {
            sentinel: sentinel.clone(),
        }
    }
}

impl Parser for SentinelParser {
    type Output = CallgrindStats;

    fn parse(&self, output: &CallgrindOutput) -> Result<Self::Output>
    where
        Self: std::marker::Sized,
    {
        debug!(
            "Parsing callgrind output file '{}' for sentinel '{}'",
            output, self.sentinel
        );

        let mut iter = output.lines()?;
        let properties = parse_header(&mut iter)
            .map_err(|error| Error::ParseError((output.0.clone(), error.to_string())))?;

        let mut found = false;
        let mut costs = properties.costs_prototype;
        let mut start_record = false;

        for line in iter.filter(|p| !p.starts_with('#')) {
            let line = line.trim();
            if line.is_empty() {
                start_record = false;
                continue;
            }
            if !start_record {
                if let Some(func) = line.strip_prefix("fn=") {
                    if self.sentinel.matches(func) {
                        {
                            trace!("Found line with sentinel: '{}'", line);
                            start_record = true;
                        }
                        found = true;
                    }
                }
                continue;
            }

            // we check if it is a line with counters and summarize them
            if line.starts_with(|c: char| c.is_ascii_digit()) {
                // From the documentation of the callgrind format:
                // > If a cost line specifies less event counts than given in the "events" line, the
                // > rest is assumed to be zero.
                trace!("Found line with counters: '{}'", line);
                costs.add_iter_str(
                    line
                    .split_ascii_whitespace()
                    // skip the positions
                    .skip(properties.positions_prototype.len()),
                );
                trace!("Updated counters to '{:?}'", &costs);
            } else {
                trace!("Skipping line: '{}'", line);
            }
        }

        if found {
            Ok(CallgrindStats(costs))
        } else {
            Err(Error::ParseError((
                output.as_path().to_owned(),
                format!("Sentinel '{}' not found", &self.sentinel),
            ))
            .into())
        }
    }
}