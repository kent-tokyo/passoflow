# Using PassoFlow

This guide is for people who operate PassoFlow through its UI instead of editing YAML directly. It explains the path from choosing an action to entering values, saving, and running a scenario.

## Start here

For a first scenario, follow this path:

1. Open or create a scenario.
2. Add an action by clicking it in the left panel or dragging it onto the canvas.
3. Select the action and fill the required fields on the right.
4. Save the scenario, then press Run.
5. Read the execution log if the result is not what you expected.

## Understanding the screen

Use the action panel on the left to find an operation and place it on the canvas in the center. The properties panel on the right contains the settings for the selected action. At the bottom, use the execution log, variables list, and imported-data view to check results.

![PassoFlow screen overview](../images/screenshot_passoflow_01.png)

_Screen overview example: actions are on the left, the flow is in the center, and the selected action's settings are on the right._

## Build the flow

1. Search for the operation you want in the search box on the left. Start with a goal such as “click a button” or “enter text”.
2. Drag the action onto the canvas. You can also click it to insert it after the selected action.
3. Select the new node and fill in required values in the properties panel on the right.
4. Drag nodes to arrange them. Follow the arrowheads on the connections to confirm execution order.

You normally build a flow from top to bottom. Keep the first version small: make
one visible action work before adding loops, branches, or table data.

## Common tasks

- Operate a button or menu: capture the target on screen and use an image-click action.
- Enter text: place a text-entry action after the action that focuses the input field.
- Start an application: place app launch, wait, and activate-window actions in order.
- Branch on a condition: add a condition branch and place each path's actions inside the true and false areas.

## Use Excel / CSV data

Import an Excel or CSV file from the “Imported data” panel at the bottom. Every row is checked by default after import; rows you uncheck are skipped during execution. The selected rows are passed through the work between “Start” and “End” one by one.

## Save and run

1. Choose “Save” from the menu or press Ctrl+S.
2. Press the Run button.
3. Check the execution log for the preflight result and each action's outcome.
4. If an image or window is not found, use the log to review the action's image, wait time, and retry count.

Before adding more actions, confirm that the current step works. This makes
image-matching and window-activation problems easier to locate.

## For advanced users

If you edit YAML directly or need the full action-parameter, variable, loop, branch, and validation specification, see the [Advanced YAML / action reference](/docs/manual?lang=en&audience=advanced).

## Browser operation modes

Choose one mode per browser flow:

- Visual mode: `open_url`, then `activate_window` and screen-image actions such as `click_image`.
- DOM mode: `browser_navigate`, `browser_click`, `browser_fill`, and `browser_wait_for` using CSS selectors in a Playwright-controlled browser.

Install Chromium once with `python -m playwright install chromium` before using DOM mode.
When the editor opens, the first-use guide checks for the Playwright package and Chromium and shows the missing setup command if needed.
