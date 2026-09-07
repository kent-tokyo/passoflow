"""Dependency-free action outcome contracts shared by runner and tests."""

from __future__ import annotations


# Actions with an intentional warning path that does not stop a scenario.
WARNING_CONTINUE_ACTIONS = frozenset(
    {
        "activate_window",
        "copy_file",
        "create_excel_sheet",
        "delete_excel_row",
        "delete_excel_sheet",
        "get_excel_value",
        "load_table",
        "map_network_drive",
        "move_file",
        "move_mouse_to_image",
        "click_image",
        "rename_file",
        "run_excel_macro",
        "save_excel_file",
        "set_excel_value",
        "sort_excel_range",
    }
)


def action_outcome_contract(action: str, params: dict | None = None) -> dict[str, bool]:
    """Describe the outcomes an action may produce without executing it."""
    if action == "send_webhook" and (params or {}).get("on_error", "continue") == "continue":
        warning_continue = True
    else:
        warning_continue = action in WARNING_CONTINUE_ACTIONS
    return {"success": True, "warning_continue": warning_continue, "failure_stop": True}
