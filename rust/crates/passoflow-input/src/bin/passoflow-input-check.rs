use std::process::ExitCode;

use passoflow_input::{
    CoordinateSpace, InputConfig, InputController, MouseButton, Point, RecordingInput, Rect,
};
use serde::Serialize;

#[derive(Serialize)]
struct DryRunReport {
    contract: &'static str,
    mode: &'static str,
    events: Vec<passoflow_input::InputEvent>,
}

fn main() -> ExitCode {
    let mut controller = InputController::new(
        RecordingInput::default(),
        InputConfig {
            screen_bounds: Some(Rect {
                left: 0,
                top: 0,
                width: 800,
                height: 600,
            }),
            fail_safe: false,
            ..InputConfig::default()
        },
    );
    if let Err(error) = run_dry_case(&mut controller) {
        eprintln!("input contract check failed: {error}");
        return ExitCode::from(1);
    }
    let events = controller.into_backend().events;
    let report = DryRunReport {
        contract: "passoflow.input.v1",
        mode: "dry_run",
        events,
    };
    match serde_json::to_string_pretty(&report) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("could not encode input report: {error}");
            ExitCode::from(2)
        }
    }
}

fn run_dry_case(
    controller: &mut InputController<RecordingInput>,
) -> Result<(), Box<dyn std::error::Error>> {
    controller.move_to(Point { x: 20, y: 30 }, CoordinateSpace::Screen)?;
    controller.click(
        Point { x: 20, y: 30 },
        CoordinateSpace::Screen,
        MouseButton::Left,
        1,
    )?;
    controller.scroll(0, -120)?;
    controller.hotkey(["ctrl", "s"])?;
    controller.type_text("PassoFlow")?;
    Ok(())
}
