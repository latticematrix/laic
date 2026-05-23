pub(crate) fn nul_digit_defaults_source() -> String {
    // Keep this source in code instead of a text fixture: embedding a literal NUL byte in a
    // checked-in `.laic` file is fragile across editors, shells, and diff tools.
    format!(
        "version \"1.0.0\";\n\nskill nul_defaults {{\n    id = \"nul-defaults\";\n\n    input NulDefaultsInput {{\n        payload: string = \"nul{nul}1tail\";\n    }}\n\n    output NulDefaultsOutput {{\n        note: string = \"ok\";\n    }}\n}}\n",
        nul = '\0'
    )
}

#[allow(dead_code)]
pub(crate) fn i64_default_source() -> String {
    "version \"1.0.0\";\n\nskill bigint_defaults {\n    id = \"bigint-defaults\";\n\n    input BigintDefaultsInput {\n        token_count: i64 = 42;\n    }\n\n    output BigintDefaultsOutput {\n        status: string;\n    }\n}\n"
        .to_string()
}
