use std::fmt::{self, Display, Formatter};

use inquire::{InquireError, Select, Text};
use crate::menu::inquire::default_select_page_size;

enum ConfigureOptions {
    Help,
}

impl Display for ConfigureOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ConfigureOptions::Help => write!(f, "❓ Help"),
        }
    }
}

pub(crate) async fn configure_settings() -> Result<(), ()> {
    let list = vec![ConfigureOptions::Help];

    let selection = Select::new("What to configure?", list)
        .with_page_size(default_select_page_size())
        .prompt();
    match selection {
        Ok(ConfigureOptions::Help) => {
            print_help();

            match Text::new("Press Enter to continue...").prompt() {
                Ok(_) | Err(InquireError::OperationCanceled) => Ok(()),
                Err(InquireError::OperationInterrupted) => Err(()),
                Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
            }
        }
        Err(InquireError::OperationCanceled) => Ok(()),
        Err(InquireError::OperationInterrupted) => Err(()),
        Err(err) => panic!("Unexpected error, try restarting the terminal: {}", err),
    }
}

fn print_help() {
    println!(
        "
    In the future you will be able to configure if core vs. non-core time is shown with the
    \"Do Now\" list. For now to find out this time you need to go into the Back Menu -> Reflection
    option and give a time range and then at the bottom of the report you will see core versus
    non-core time. For example enter \"2d\" and then \"0m\" to see the last two days of time.

    Also in the future you will be able to configure how the default selection is made when there 
    are multiple choices in a priority list. Until then the recommendation is to set a goal for
    how much time you want to spend on core versus non-core work and then to generally favor core
    work or non-core work based on this goal. One strategy is to always favor core work in the 
    urgent categories (🔥 & 🔴) and then when on the importance & maybe urgent category (🔝 & 🟡) 
    go based on if you are above or below your goal of core work. Then you can make sure that you
    are able to get core work done without neglecting non-core work. Remember that you can quickly 
    scan for if work is core or non-core by looking to the end of the item printed pay attention to
    the core unicode of a building🏢 or non-core unicode of a broom🧹.
    "
    );
}
