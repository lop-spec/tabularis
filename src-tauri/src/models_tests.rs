#[cfg(test)]
mod tests {
    use crate::models::{single_db_before_multi_transition, DatabaseSelection};

    #[test]
    fn single_to_multi_returns_previous_name() {
        let previous = DatabaseSelection::Single("app".into());
        let new = DatabaseSelection::Multiple(vec!["app".into(), "logs".into()]);
        assert_eq!(
            single_db_before_multi_transition(&previous, &new),
            Some("app".into())
        );
    }

    #[test]
    fn multiple_with_one_element_treated_as_single() {
        let previous = DatabaseSelection::Multiple(vec!["app".into()]);
        let new = DatabaseSelection::Multiple(vec!["app".into(), "logs".into()]);
        assert_eq!(
            single_db_before_multi_transition(&previous, &new),
            Some("app".into())
        );
    }

    #[test]
    fn multi_to_multi_returns_none() {
        let previous = DatabaseSelection::Multiple(vec!["a".into(), "b".into()]);
        let new = DatabaseSelection::Multiple(vec!["a".into(), "b".into(), "c".into()]);
        assert_eq!(single_db_before_multi_transition(&previous, &new), None);
    }

    #[test]
    fn single_to_single_returns_none() {
        let previous = DatabaseSelection::Single("a".into());
        let new = DatabaseSelection::Single("b".into());
        assert_eq!(single_db_before_multi_transition(&previous, &new), None);
    }

    #[test]
    fn single_to_multiple_with_one_item_returns_none() {
        let previous = DatabaseSelection::Single("app".into());
        let new = DatabaseSelection::Multiple(vec!["app".into()]);
        assert_eq!(single_db_before_multi_transition(&previous, &new), None);
    }

    #[test]
    fn empty_previous_name_returns_none() {
        let previous = DatabaseSelection::Single("".into());
        let new = DatabaseSelection::Multiple(vec!["a".into(), "b".into()]);
        assert_eq!(single_db_before_multi_transition(&previous, &new), None);
    }

    #[test]
    fn whitespace_previous_name_is_ignored() {
        let previous = DatabaseSelection::Single("   ".into());
        let new = DatabaseSelection::Multiple(vec!["a".into(), "b".into()]);
        assert_eq!(single_db_before_multi_transition(&previous, &new), None);
    }
}
