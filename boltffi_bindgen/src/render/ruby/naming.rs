use heck::{ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase};

pub fn method_name(s: &str) -> String {
    let snake = s.to_snake_case();
    if is_reserved(&snake) {
        format!("{snake}_")
    } else {
        snake
    }
}

pub fn class_name(s: &str) -> String {
    s.to_upper_camel_case()
}

pub fn const_name(s: &str) -> String {
    s.to_shouty_snake_case()
}

fn is_reserved(s: &str) -> bool {
    matches!(
        s,
        "begin"
            | "end"
            | "class"
            | "module"
            | "def"
            | "do"
            | "if"
            | "else"
            | "elsif"
            | "unless"
            | "while"
            | "until"
            | "for"
            | "in"
            | "return"
            | "yield"
            | "self"
            | "nil"
            | "true"
            | "false"
            | "and"
            | "or"
            | "not"
            | "send"
            | "raise"
            | "rescue"
            | "ensure"
            | "retry"
            | "redo"
            | "super"
            | "then"
            | "case"
            | "when"
            | "break"
            | "next"
            | "alias"
            | "defined"
            | "lambda"
            | "proc"
            | "block"
    )
}
