extern crate lalrpop;
fn main() {
    lalrpop::Configuration::new().log_verbose().process_current_dir().unwrap();
}
