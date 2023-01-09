use std::collections::BTreeMap;

use actix_web::{get, post, web, App, HttpServer, Responder, HttpResponse};
use damasc::{env::Environment, statement::Statement, expression::ExpressionSet};
use serde::Deserialize;
use damasc::parser::statement;
use askama::Template;

#[derive(Deserialize)]
struct Repl {
    statement: String,
}


#[derive(Template)]
#[template(path = "result.html")]                      
struct ResultTemplate<'x> {
    repl: &'x Repl,
    error: Option<String>,
    output: Option<String>,
}

#[derive(Template)]
#[template(path = "index.html")]                      
struct HomeTemplate {
}

#[post("/")]
async fn eval(repl: web::Form<Repl>) -> impl Responder {
    let env = Environment {
        bindings: BTreeMap::new(),
    };

    match statement(&repl.statement) {
        Ok((_, s)) => {
            let output = match s  {
                Statement::Clear => {Some("".to_owned())},
                Statement::Exit => {Some("".to_owned())},
                Statement::Help => {Some("".to_owned())},
                Statement::Inspect(_) => {Some("".to_owned())},
                Statement::Format(_) => {Some("".to_owned())},
                Statement::Eval(ExpressionSet{expressions}) => {
                    Some(expressions.iter().map(|e| {
                        match env.eval_expr(e) {
                            Ok(r) => format!("{r}"),
                            Err(err) => {
                                format!("Eval Error, {err:?}")
                            }
                        }
                    }).collect::<Vec<String>>().join(";"))
                },
                Statement::Literal(_) => {Some("".to_owned())},
                Statement::Pattern(_) => {Some("".to_owned())},
                Statement::AssignSet(_) => {Some("".to_owned())},
                Statement::MatchSet(_) => {Some("".to_owned())},
                Statement::Insert(_) => {Some("".to_owned())},
                Statement::Pop(_) => {Some("".to_owned())},
                Statement::Query(_) => {Some("".to_owned())},
                Statement::Deletion(_) => {Some("".to_owned())},
                Statement::Import(_) => {Some("".to_owned())},
                Statement::Export(_) => {Some("".to_owned())},
                Statement::UseBag(_, _) => {Some("".to_owned())},
                Statement::TellBag => {Some("".to_owned())},
            };
            
            ResultTemplate{
                error: None,
                repl: &repl,
                output,
            }
        },
        Err(e) => {
            ResultTemplate{
                error: Some(e.to_string()),
                repl: &repl,
                output: None,
            }
        }
    }.render().map(|s| HttpResponse::Ok().content_type("text/html").body(s)).unwrap_or_else(template_error)
}

fn template_error(_ : askama::Error) -> HttpResponse {
    HttpResponse::InternalServerError().content_type("text/html").body("Template Error")
}

#[get("/")]
async fn home() -> impl Responder {
    HomeTemplate{}.render().map(|s| HttpResponse::Ok().content_type("text/html").body(s)).unwrap_or_else(template_error)
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    let server = HttpServer::new(|| {
        App::new().service(home).service(eval)
    })
    .bind(("127.0.0.1", 8080))?;

    println!("Server started: http://127.0.0.1:8080");
    
    server.run()
    .await
}