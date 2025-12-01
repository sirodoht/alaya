use askama::Template;

use crate::books::Book;

#[derive(Template)]
#[template(path = "book_list.html")]
pub struct BookListTemplate {
    pub is_authenticated: bool,
    pub signups_disabled: bool,
    pub username: String,
    pub books: Vec<Book>,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub is_authenticated: bool,
    pub signups_disabled: bool,
    pub username: String,
    pub form_username: String,
    pub error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "signup.html")]
pub struct SignupTemplate {
    pub is_authenticated: bool,
    pub signups_disabled: bool,
    pub username: String,
    pub form_username: String,
    pub error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "book_form.html")]
pub struct BookFormTemplate {
    pub is_authenticated: bool,
    pub signups_disabled: bool,
    pub username: String,
    pub error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "book_quick_add.html")]
pub struct QuickAddTemplate {
    pub is_authenticated: bool,
    pub signups_disabled: bool,
    pub username: String,
    pub error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "book_detail.html")]
pub struct BookDetailTemplate {
    pub is_authenticated: bool,
    pub signups_disabled: bool,
    pub username: String,
    pub book: Book,
}

#[derive(Template)]
#[template(path = "profile.html")]
pub struct ProfileTemplate {
    pub is_authenticated: bool,
    pub signups_disabled: bool,
    pub username: String,
    pub book_count: i64,
}
