pub const ROBOTS_TRIGGER_LINK_SLOT_COUNT: usize = 8;
pub const ROBOTS_TRIGGER_LINK7_ORDINAL: usize = 7;

/// Shared native-compatible validation for serialized/runtime EXTrigger links.
/// Negative sentinels and indices outside the current trigger table are invalid.
pub fn robots_trigger_link_index(link: i32, trigger_count: usize) -> Option<usize> {
    let index = usize::try_from(link).ok()?;
    (index < trigger_count).then_some(index)
}

/// Follow one fixed EXTrigger link ordinal for an exact number of hops.
///
/// `read_link(trigger_index, ordinal)` returns the raw signed link index stored by
/// the host representation. The helper owns the native 8-slot/negative/out-of-range
/// rules while GUI/UE remain free to store their graph however they want.
pub fn robots_follow_trigger_link_chain<F>(
    start_trigger_index: usize,
    trigger_count: usize,
    link_ordinal: usize,
    hops: usize,
    mut read_link: F,
) -> Option<usize>
where
    F: FnMut(usize, usize) -> Option<i32>,
{
    if link_ordinal >= ROBOTS_TRIGGER_LINK_SLOT_COUNT {
        return None;
    }

    let mut current = start_trigger_index;
    if current >= trigger_count {
        return None;
    }

    for _ in 0..hops {
        let raw_link = read_link(current, link_ordinal)?;
        current = robots_trigger_link_index(raw_link, trigger_count)?;
    }
    Some(current)
}

/// Native BossSewer helper `0x00484C00(index)` semantics: begin at the creator
/// trigger and follow EXTrigger link7 exactly `index + 1` times.
pub fn robots_follow_trigger_link7_stage_chain<F>(
    creator_trigger_index: usize,
    trigger_count: usize,
    stage_chain_index: u8,
    read_link: F,
) -> Option<usize>
where
    F: FnMut(usize, usize) -> Option<i32>,
{
    robots_follow_trigger_link_chain(
        creator_trigger_index,
        trigger_count,
        ROBOTS_TRIGGER_LINK7_ORDINAL,
        usize::from(stage_chain_index) + 1,
        read_link,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_link_validation_matches_gui_and_cli_contract() {
        assert_eq!(robots_trigger_link_index(-1, 4), None);
        assert_eq!(robots_trigger_link_index(-2, 4), None);
        assert_eq!(robots_trigger_link_index(0, 4), Some(0));
        assert_eq!(robots_trigger_link_index(3, 4), Some(3));
        assert_eq!(robots_trigger_link_index(4, 4), None);
    }

    #[test]
    fn generic_chain_uses_exact_fixed_ordinal_and_hop_count() {
        let links = [
            [1, -1, -1, -1, -1, -1, -1, 2],
            [-1; 8],
            [-1, -1, -1, -1, -1, -1, -1, 3],
            [-1; 8],
        ];
        let result = robots_follow_trigger_link_chain(0, links.len(), 7, 2, |node, ordinal| {
            links.get(node).and_then(|row| row.get(ordinal)).copied()
        });
        assert_eq!(result, Some(3));
    }

    #[test]
    fn chain_fails_closed_on_invalid_ordinal_missing_link_or_bad_start() {
        let links = [[-1; 8]; 2];
        assert_eq!(
            robots_follow_trigger_link_chain(0, links.len(), 8, 1, |node, ordinal| {
                links.get(node).and_then(|row| row.get(ordinal)).copied()
            }),
            None
        );
        assert_eq!(
            robots_follow_trigger_link_chain(0, links.len(), 7, 1, |node, ordinal| {
                links.get(node).and_then(|row| row.get(ordinal)).copied()
            }),
            None
        );
        assert_eq!(
            robots_follow_trigger_link_chain(2, links.len(), 7, 0, |node, ordinal| {
                links.get(node).and_then(|row| row.get(ordinal)).copied()
            }),
            None
        );
    }

    #[test]
    fn sewer_stage_index_is_one_based_hop_count_over_link7() {
        let mut links = [[-1; 8]; 6];
        links[0][7] = 1;
        links[1][7] = 2;
        links[2][7] = 3;
        links[3][7] = 4;
        links[4][7] = 5;

        let read =
            |node: usize, ordinal: usize| links.get(node).and_then(|row| row.get(ordinal)).copied();
        assert_eq!(
            robots_follow_trigger_link7_stage_chain(0, links.len(), 0, read),
            Some(1)
        );
        assert_eq!(
            robots_follow_trigger_link7_stage_chain(0, links.len(), 4, read),
            Some(5)
        );
    }
}
