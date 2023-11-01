use std::borrow::Cow;
use std::fmt::{Display, Write};

use anyhow::Result;
use colored::{ColoredString, Colorize};

use super::callgrind::model::Costs;
use crate::api::EventKind;
use crate::util::{factor_diff, percentage_diff, to_string_signed_short, truncate_str_utf8};

pub struct Header {
    pub module_path: String,
    pub id: Option<String>,
    pub description: Option<String>,
}

pub trait Formatter {
    fn format_float(float: f64, unit: &str) -> ColoredString {
        let signed_short = to_string_signed_short(float);
        if float.is_infinite() {
            if float.is_sign_positive() {
                format!("{signed_short:+^9}").bright_red().bold()
            } else {
                format!("{signed_short:-^9}").bright_green().bold()
            }
        } else if float.is_sign_positive() {
            format!("{signed_short:^+8}{unit}").bright_red().bold()
        } else {
            format!("{signed_short:^+8}{unit}").bright_green().bold()
        }
    }
    fn format(&self, new_costs: &Costs, old_costs: Option<&Costs>) -> Result<String>;
}

#[derive(Clone)]
pub struct VerticalFormat {
    event_kinds: Vec<EventKind>,
}

impl Header {
    pub fn new<T, U, V>(module_path: T, id: U, description: V) -> Self
    where
        T: Into<String>,
        U: Into<Option<String>>,
        V: Into<Option<String>>,
    {
        Self {
            module_path: module_path.into(),
            id: id.into(),
            description: description.into(),
        }
    }

    pub fn from_segments<I, T, U, V>(module_path: T, id: U, description: V) -> Self
    where
        I: AsRef<str>,
        T: AsRef<[I]>,
        U: Into<Option<String>>,
        V: Into<Option<String>>,
    {
        Self {
            module_path: module_path
                .as_ref()
                .iter()
                .map(|s| s.as_ref().to_owned())
                .collect::<Vec<String>>()
                .join("::"),
            id: id.into(),
            description: description.into(),
        }
    }

    pub fn print(&self) {
        println!("{self}");
    }

    pub fn to_title(&self) -> String {
        let mut output = String::new();
        write!(&mut output, "{}", self.module_path).unwrap();
        if let Some(id) = &self.id {
            if let Some(description) = &self.description {
                let truncated = truncate_str_utf8(description, 37);
                write!(
                    &mut output,
                    " {id}:{truncated}{}",
                    if truncated.len() < description.len() {
                        "..."
                    } else {
                        ""
                    }
                )
                .unwrap();
            } else {
                write!(&mut output, " {id}").unwrap();
            }
        }
        output
    }
}

impl Display for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.module_path.green()))?;
        if let Some(id) = &self.id {
            if let Some(description) = &self.description {
                let truncated = truncate_str_utf8(description, 37);
                f.write_fmt(format_args!(
                    " {}{}{}{}",
                    id.cyan(),
                    ":".cyan(),
                    truncated.bold().blue(),
                    if truncated.len() < description.len() {
                        "..."
                    } else {
                        ""
                    }
                ))?;
            } else {
                f.write_fmt(format_args!(" {}", id.cyan()))?;
            }
        }
        Ok(())
    }
}

impl Default for VerticalFormat {
    fn default() -> Self {
        use EventKind::*;
        Self {
            event_kinds: vec![
                Ir,
                L1hits,
                LLhits,
                RamHits,
                TotalRW,
                EstimatedCycles,
                SysCount,
                SysTime,
                SysCpuTime,
                Ge,
                Bc,
                Bcm,
                Bi,
                Bim,
                ILdmr,
                DLdmr,
                DLdmw,
                AcCost1,
                AcCost2,
                SpLoss1,
                SpLoss2,
            ],
        }
    }
}

impl Formatter for VerticalFormat {
    fn format(&self, new_costs: &Costs, old_costs: Option<&Costs>) -> Result<String> {
        let mut new_costs = Cow::Borrowed(new_costs);
        let mut old_costs = old_costs.map(Cow::Borrowed);
        let mut result = String::new();

        let not_available = "N/A";
        let unknown = "*********";
        let no_change = "No change";

        for event_kind in &self.event_kinds {
            if event_kind.is_derived() {
                if !new_costs.is_summarized() {
                    _ = new_costs.to_mut().make_summary();
                }
                if !old_costs.as_ref().map_or(true, |c| c.is_summarized()) {
                    _ = old_costs.as_mut().map(|c| c.to_mut().make_summary());
                }
            }
            let description = match event_kind {
                EventKind::Ir => "Instructions:".to_owned(),
                EventKind::L1hits => "L1 Hits:".to_owned(),
                EventKind::LLhits => "L2 Hits:".to_owned(),
                EventKind::RamHits => "RAM Hits:".to_owned(),
                EventKind::TotalRW => "Total read+write:".to_owned(),
                EventKind::EstimatedCycles => "Estimated Cycles:".to_owned(),
                event_kind => format!("{event_kind}:"),
            };
            match (
                new_costs.cost_by_kind(event_kind),
                old_costs.as_ref().and_then(|c| c.cost_by_kind(event_kind)),
            ) {
                (None, Some(old_cost)) => writeln!(
                    result,
                    "  {description:<18}{:>15}|{old_cost:<15} ({:^9})",
                    not_available.bold(),
                    unknown.bright_black()
                )?,
                (Some(new_cost), None) => writeln!(
                    result,
                    "  {description:<18}{:>15}|{not_available:<15} ({:^9})",
                    new_cost.to_string().bold(),
                    unknown.bright_black()
                )?,
                (Some(new_cost), Some(old_cost)) if new_cost == old_cost => writeln!(
                    result,
                    "  {description:<18}{:>15}|{old_cost:<15} ({:^9})",
                    new_cost.to_string().bold(),
                    no_change.bright_black()
                )?,
                (Some(new_cost), Some(old_cost)) => {
                    let pct_string = {
                        let pct = percentage_diff(new_cost, old_cost);
                        Self::format_float(pct, "%")
                    };
                    let factor_string = {
                        let factor = factor_diff(new_cost, old_cost);
                        Self::format_float(factor, "x")
                    };
                    writeln!(
                        result,
                        "  {description:<18}{:>15}|{old_cost:<15} ({pct_string:^9}) \
                         [{factor_string:^9}]",
                        new_cost.to_string().bold(),
                    )?;
                }
                _ => {}
            }
        }
        Ok(result)
    }
}
