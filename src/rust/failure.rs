use std::fmt::{self, Display, Debug};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FailStage {
    Startup,
    Runtime,
}

impl FailStage {
    fn start_err_msg(self) -> &'static str {
        match self {
            FailStage::Startup => "Screeps Conrod Client failed to start.",
            FailStage::Runtime => "Screeps Conrod Client crashed.",
        }
    }
}

enum DisplayOrDebug<T1 = &'static str, T2 = &'static str>
    where T1: Display,
          T2: Debug
{
    Display(T1),
    Debug(T2),
}

impl<T1: Display, T2: Debug> Display for DisplayOrDebug<T1, T2> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match *self {
            DisplayOrDebug::Display(ref v) => Display::fmt(v, f),
            DisplayOrDebug::Debug(ref v) => write!(f, "debug info: {:?}", v),
        }
    }
}

/// Fail nicely with a nice err message.
fn fail_gracefully<T1, T2>(stage: FailStage, err: Option<DisplayOrDebug<T1, T2>>, message: &str) -> !
    where T1: Display,
          T2: Debug
{
    match err {
        Some(e) => {
            panic!(r#"
{}

Error details:

{}

Specific error to occur: {}"#,
                   stage.start_err_msg(),
                   message,
                   e);
        }
        None => {
            panic!(r#"
{}

Error details:

{}"#,
                   stage.start_err_msg(),
                   message);
        }
    }
}

pub trait FailureUnwrap {
    type Target;
    fn uw(self, stage: FailStage, msg: &str) -> Self::Target;
}

pub trait FailureUnwrapDebug {
    type Target;
    fn uwd(self, stage: FailStage, msg: &str) -> Self::Target;
}

impl<T> FailureUnwrap for Option<T> {
    type Target = T;
    fn uw(self, stage: FailStage, msg: &str) -> T {
        match self {
            Some(v) => v,
            None => fail_gracefully::<&'static str, &'static str>(stage, None, msg),
        }
    }
}

impl<T, E> FailureUnwrap for Result<T, E>
    where E: Display
{
    type Target = T;
    fn uw(self, stage: FailStage, msg: &str) -> T {
        match self {
            Ok(v) => v,
            Err(e) => fail_gracefully::<E, &str>(stage, Some(DisplayOrDebug::Display(e)), msg),
        }
    }
}

impl<T, E> FailureUnwrapDebug for Result<T, E>
    where E: Debug
{
    type Target = T;
    fn uwd(self, stage: FailStage, msg: &str) -> T {
        match self {
            Ok(v) => v,
            Err(e) => fail_gracefully::<&str, E>(stage, Some(DisplayOrDebug::Debug(e)), msg),
        }
    }
}
