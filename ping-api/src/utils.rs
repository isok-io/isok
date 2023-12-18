use std::str::FromStr;

pub fn env_panic(env: &'static str) -> impl Fn(std::env::VarError) -> String {
    move |e| {
        panic!("{env} is not set ({})", e);
    }
}

pub fn env_get(env: &'static str) -> String {
    std::env::var(env).unwrap_or_else(env_panic(env))
}

pub fn env_parse_panic<T: FromStr>(env: &'static str, val: String) -> impl Fn(T::Err) -> T {
    move |_| {
        panic!("can't parse {env} ({val})");
    }
}

pub fn env_get_num<T: FromStr>(env: &'static str, other: T) -> T {
    match std::env::var(env) {
        Ok(v) => v.parse::<T>().unwrap_or_else(env_parse_panic::<T>(env, v)),
        Err(_) => other,
    }
}