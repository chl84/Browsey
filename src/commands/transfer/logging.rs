use super::error::TransferResult;
use super::route::{route_hint_label, MixedRouteHint};
use super::{MixedTransferConflictInfo, MixedTransferOp};
use std::time::Instant;
use tracing::{info, warn};

pub(super) fn log_mixed_preview_result(
    result: &TransferResult<Vec<MixedTransferConflictInfo>>,
    route_hint: MixedRouteHint,
    source_count: usize,
    started: Instant,
) {
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(conflicts) => info!(
            op = "mixed_conflict_preview",
            route = route_hint_label(route_hint),
            source_count,
            conflicts = conflicts.len(),
            elapsed_ms,
            "mixed conflict preview completed"
        ),
        Err(err) => warn!(
            op = "mixed_conflict_preview",
            route = route_hint_label(route_hint),
            source_count,
            elapsed_ms,
            error_code = err.code_str(),
            error_message = err.message(),
            "mixed conflict preview failed"
        ),
    }
}

pub(super) fn log_mixed_execute_result(
    op: MixedTransferOp,
    result: &TransferResult<Vec<String>>,
    route_hint: MixedRouteHint,
    source_count: usize,
    started: Instant,
) {
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let op_name = match op {
        MixedTransferOp::Copy => "mixed_write_copy",
        MixedTransferOp::Move => "mixed_write_move",
    };
    match result {
        Ok(created) => info!(
            op = op_name,
            route = route_hint_label(route_hint),
            source_count,
            outputs = created.len(),
            elapsed_ms,
            "mixed transfer completed"
        ),
        Err(err) => warn!(
            op = op_name,
            route = route_hint_label(route_hint),
            source_count,
            elapsed_ms,
            error_code = err.code_str(),
            error_message = err.message(),
            "mixed transfer failed"
        ),
    }
}

pub(super) fn log_mixed_single_execute_result(
    op: MixedTransferOp,
    result: &TransferResult<String>,
    route_hint: MixedRouteHint,
    started: Instant,
) {
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let op_name = match op {
        MixedTransferOp::Copy => "mixed_write_copy",
        MixedTransferOp::Move => "mixed_write_move",
    };
    match result {
        Ok(_) => info!(
            op = op_name,
            route = route_hint_label(route_hint),
            source_count = 1usize,
            outputs = 1usize,
            elapsed_ms,
            "mixed transfer completed"
        ),
        Err(err) => warn!(
            op = op_name,
            route = route_hint_label(route_hint),
            source_count = 1usize,
            elapsed_ms,
            error_code = err.code_str(),
            error_message = err.message(),
            "mixed transfer failed"
        ),
    }
}
