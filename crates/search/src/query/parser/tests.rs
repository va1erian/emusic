use super::*;

fn word(text: &str) -> Term {
    Term::text(TextMatch::word(text))
}

fn phrase(text: &str) -> Term {
    Term::text(TextMatch::phrase(text))
}

fn cmp(op: Comparison, value: f64) -> FieldValue {
    FieldValue::Numeric(NumericSpec::Compare { op, value })
}

fn range(min: Option<f64>, max: Option<f64>) -> FieldValue {
    FieldValue::Numeric(NumericSpec::Range { min, max })
}

#[test]
fn empty_input_has_no_terms() {
    assert_eq!(parse("").terms, vec![]);
}

#[test]
fn whitespace_only_has_no_terms() {
    assert_eq!(parse("   \t\n ").terms, vec![]);
}

#[test]
fn single_word() {
    assert_eq!(parse("foo").terms, vec![word("foo")]);
}

#[test]
fn multiple_words_are_anded_terms() {
    assert_eq!(parse("foo bar").terms, vec![word("foo"), word("bar")]);
}

#[test]
fn exact_phrase() {
    assert_eq!(parse(r#""foo bar""#).terms, vec![phrase("foo bar")]);
}

#[test]
fn stray_quote_becomes_phrase() {
    assert_eq!(parse("\"unclosed").terms, vec![phrase("unclosed")]);
}

#[test]
fn empty_quotes_are_dropped() {
    assert_eq!(parse(r#"foo "" bar"#).terms, vec![word("foo"), word("bar")]);
}

#[test]
fn text_field_filter() {
    assert_eq!(
        parse("artist:foo").terms,
        vec![Term::field(
            Field::Artist,
            FieldValue::Text(TextMatch::word("foo"))
        )]
    );
}

#[test]
fn quoted_text_field_value() {
    assert_eq!(
        parse(r#"artist:"foo bar""#).terms,
        vec![Term::field(
            Field::Artist,
            FieldValue::Text(TextMatch::phrase("foo bar"))
        )]
    );
}

#[test]
fn all_text_fields_are_recognized() {
    let cases = [
        ("artist", Field::Artist),
        ("album", Field::Album),
        ("albumartist", Field::AlbumArtist),
        ("album_artist", Field::AlbumArtist),
        ("title", Field::Title),
        ("genre", Field::Genre),
        ("file", Field::File),
        ("dir", Field::Dir),
        ("ext", Field::Ext),
        ("comment", Field::Comment),
        ("composer", Field::Composer),
    ];
    for (name, field) in cases {
        let q = parse(&format!("{name}:x"));
        assert_eq!(
            q.terms,
            vec![Term::field(field, FieldValue::Text(TextMatch::word("x")))],
            "field {name} not recognized"
        );
    }
}

#[test]
fn field_names_are_case_insensitive() {
    assert_eq!(
        parse("ARTIST:foo").terms,
        vec![Term::field(
            Field::Artist,
            FieldValue::Text(TextMatch::word("foo"))
        )]
    );
}

#[test]
fn numeric_value_on_text_field_stays_text() {
    assert_eq!(
        parse("artist:1994").terms,
        vec![Term::field(
            Field::Artist,
            FieldValue::Text(TextMatch::word("1994"))
        )]
    );
}

#[test]
fn unknown_field_prefix_is_plain_text() {
    assert_eq!(parse("foo:bar").terms, vec![word("foo:bar")]);
}

#[test]
fn unknown_field_prefix_with_quoted_value_is_plain_phrase() {
    assert_eq!(parse(r#"foo:"bar baz""#).terms, vec![phrase("foo:bar baz")]);
}

#[test]
fn unknown_field_prefix_with_empty_value_is_dropped() {
    assert_eq!(parse("foo:").terms, vec![]);
}

#[test]
fn negated_word() {
    assert_eq!(
        parse("-foo").terms,
        vec![Term::negated_text(TextMatch::word("foo"))]
    );
}

#[test]
fn negated_field_filter() {
    assert_eq!(
        parse("-artist:foo").terms,
        vec![Term::negated_field(
            Field::Artist,
            FieldValue::Text(TextMatch::word("foo"))
        )]
    );
}

#[test]
fn negated_phrase() {
    assert_eq!(
        parse(r#"-"foo bar""#).terms,
        vec![Term::negated_text(TextMatch::phrase("foo bar"))]
    );
}

#[test]
fn year_bare_value_is_equal() {
    assert_eq!(
        parse("year:1994").terms,
        vec![Term::field(Field::Year, cmp(Comparison::Equal, 1994.0))]
    );
}

#[test]
fn year_closed_range() {
    assert_eq!(
        parse("year:1990..1999").terms,
        vec![Term::field(Field::Year, range(Some(1990.0), Some(1999.0)))]
    );
}

#[test]
fn year_open_ranges() {
    assert_eq!(
        parse("year:..1999").terms,
        vec![Term::field(Field::Year, range(None, Some(1999.0)))]
    );
    assert_eq!(
        parse("year:1990..").terms,
        vec![Term::field(Field::Year, range(Some(1990.0), None))]
    );
}

#[test]
fn year_empty_range_falls_back_to_text() {
    assert_eq!(parse("year:..").terms, vec![word("year:..")]);
}

#[test]
fn plays_comparison_operators() {
    let cases = [
        ("plays:>10", Comparison::Greater, 10.0),
        ("plays:>=5", Comparison::GreaterEqual, 5.0),
        ("plays:<2", Comparison::Less, 2.0),
        ("plays:<=3", Comparison::LessEqual, 3.0),
        ("plays:=4", Comparison::Equal, 4.0),
        ("plays:4", Comparison::Equal, 4.0),
    ];
    for (input, op, value) in cases {
        assert_eq!(
            parse(input).terms,
            vec![Term::field(Field::Plays, cmp(op, value))],
            "input {input}"
        );
    }
}

#[test]
fn duration_units_scale_to_seconds() {
    let cases = [
        ("duration:>5m", Comparison::Greater, 300.0),
        ("duration:90s", Comparison::Equal, 90.0),
        ("duration:2h", Comparison::Equal, 7200.0),
        ("duration:5min", Comparison::Equal, 300.0),
        ("duration:1hr", Comparison::Equal, 3600.0),
        ("duration:45", Comparison::Equal, 45.0),
    ];
    for (input, op, expected) in cases {
        let q = parse(input);
        assert_eq!(
            q.terms,
            vec![Term::field(Field::Duration, cmp(op, expected))],
            "input {input}"
        );
    }
}

#[test]
fn duration_range_with_units() {
    assert_eq!(
        parse("duration:1m..5m").terms,
        vec![Term::field(Field::Duration, range(Some(60.0), Some(300.0)))]
    );
}

#[test]
fn numeric_field_with_bad_value_falls_back_to_text() {
    assert_eq!(parse("year:abc").terms, vec![word("year:abc")]);
    assert_eq!(parse("plays:>x").terms, vec![word("plays:>x")]);
    assert_eq!(parse("duration:5x").terms, vec![word("duration:5x")]);
}

#[test]
fn text_is_normalized() {
    assert_eq!(parse("Café").terms, vec![word("cafe")]);
    assert_eq!(
        parse("artist:Émile").terms,
        vec![Term::field(
            Field::Artist,
            FieldValue::Text(TextMatch::word("emile"))
        )]
    );
    assert_eq!(
        parse(r#"title:"Âme Strong""#).terms,
        vec![Term::field(
            Field::Title,
            FieldValue::Text(TextMatch::phrase("ame strong"))
        )]
    );
}

#[test]
fn complex_mixed_query() {
    let q = parse(r#"artist:"daft punk" -live year:1990..1999 plays:>10 ext:flac"#);
    assert_eq!(
        q.terms,
        vec![
            Term::field(
                Field::Artist,
                FieldValue::Text(TextMatch::phrase("daft punk"))
            ),
            Term::negated_text(TextMatch::word("live")),
            Term::field(Field::Year, range(Some(1990.0), Some(1999.0))),
            Term::field(Field::Plays, cmp(Comparison::Greater, 10.0)),
            Term::field(Field::Ext, FieldValue::Text(TextMatch::word("flac"))),
        ]
    );
}
