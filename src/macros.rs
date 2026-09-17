macro_rules! interpolate_str {
    ($template:expr, $($name:ident = $value:expr),* $(,)?) => {{
        let mut result = $template.to_owned();

        $(
            result = result.replace(
                concat!("{", stringify!($name), "}"),
                &$value.to_string(),
            );
        )*

        result
    }};
}

pub(crate) use interpolate_str;
