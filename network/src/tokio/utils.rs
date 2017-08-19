use futures::Future;

use screeps_api::{self, NoToken, TokenStorage};

use hyper;

use request::LoginDetails;

pub trait HasClient<C, H, T>
where
    C: hyper::client::Connect,
    H: screeps_api::HyperClient<C>,
    T: TokenStorage,
{
    fn login(&self) -> &LoginDetails;
    fn api(&self) -> &screeps_api::Api<C, H, T>;
}

pub fn execute_or_login_and_execute<
    Executor,
    Tokens,
    HyperConnect,
    HyperClient,
    ReturnData,
    ReturnError,
    Function,
    FunctionReturn,
    FailureFunction,
    FailureReturn,
>(
    executor: Executor,
    mut func: Function,
    mut failure_func: FailureFunction,
) -> Box<Future<Item = ReturnData, Error = ReturnError>>
where
    HyperConnect: hyper::client::Connect + 'static,
    HyperClient: screeps_api::HyperClient<HyperConnect> + 'static,
    Tokens: TokenStorage + 'static,
    Executor: HasClient<HyperConnect, HyperClient, Tokens> + 'static,
    ReturnData: 'static,
    ReturnError: 'static,
    Function: FnMut(Executor) -> Result<FunctionReturn, (Executor, NoToken)> + 'static,
    FunctionReturn: Future<Item = ReturnData, Error = ReturnError> + 'static,
    FailureFunction: FnMut(Executor, screeps_api::Error) -> FailureReturn + 'static,
    FailureReturn: Future<Item = ReturnData, Error = ReturnError> + 'static,
{
    match func(executor) {
        Ok(future) => Box::new(future) as Box<Future<Item = _, Error = _>>,
        Err((executor, NoToken)) => {
            Box::new(
                executor
                    .api()
                    .login(executor.login().username(), executor.login().password())
                    .then(move |login_result| {
                        match login_result {
                            Ok(login_ok) => {
                                login_ok.return_to(&executor.api().tokens);
                                debug!("execute_or_login_and_execute login finished, attempting to execute again.");
                                // TODO: something here to ensure that this doesn't end up as an infinite loop
                                Box::new(execute_or_login_and_execute(executor, func, failure_func)) as
                                    Box<Future<Item = _, Error = _>>
                            }
                            Err(e) => Box::new(failure_func(executor, e)) as Box<Future<Item = _, Error = _>>,
                        }
                    }),
            )
        }
    }
}
