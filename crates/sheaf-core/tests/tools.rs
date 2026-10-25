use sheaf_core::tools::*;

#[test]
fn catalogue_is_complete_and_consistent() {
    let c = catalogue();
    assert_eq!(c.len(), 29);
    let mut ids: Vec<_> = c.iter().map(|t| format!("{:?}", t.id)).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 29);
    assert!(c.iter().all(|t| !t.accepts.is_empty() && !t.description.is_empty()));
    assert_eq!(c.iter().filter(|t| t.wave == 1).count(), 29);
    assert!(c.iter().all(|t| !t.name.to_lowercase().contains("ilove")));
}

#[test]
fn every_category_has_a_tool() {
    for cat in Category::ALL {
        assert!(
            catalogue().iter().any(|t| t.category == cat),
            "no tool in {}",
            cat.label()
        );
    }
}

#[test]
fn find_returns_the_matching_tool() {
    assert_eq!(find(ToolId::Merge).name, "Merge PDF");
}

#[test]
fn search_ranks_names_then_descriptions_then_letters_in_order() {
    use sheaf_core::tools::search;
    let names = |q: &str| search(q).iter().map(|t| t.name).collect::<Vec<_>>();
    assert!(names("").is_empty() && names("   ").is_empty());
    assert_eq!(names("compress").first(), Some(&"Compress PDF"));
    assert_eq!(names("cmprs").first(), Some(&"Compress PDF"));
    // A name starting with the query comes before one only containing it.
    let merge = names("merge");
    assert_eq!(merge.first(), Some(&"Merge PDF"));
    assert!(names("zzzz").is_empty());
}
