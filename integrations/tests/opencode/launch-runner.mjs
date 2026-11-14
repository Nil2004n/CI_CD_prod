// Child process standing in for the OpenCode TUI: loads the real tui.ts,
// runs one dashboard scenario, and writes the outcome to a result file.
// Anything a PWT subprocess leaks onto this process's stdout/stderr is what
// would be painted over the TUI prompt; the test asserts there is none.
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { pathToFileURL } from "node:url";

const [, , moduleFile, scenarioFile, resultFile] = process.argv;
const scenario = JSON.parse(readFileSync(scenarioFile, "utf8"));
const { launchDashboard } = await import(pathToFileURL(moduleFile).href);

if (scenario.path !== undefined) {
  process.env.PATH = scenario.path;
}
let failure = null;
const failed = new Promise((resolve) => {
  scenario.expectFailureCallback ? setTimeout(resolve, 10_000) : resolve();
});
const message = await launchDashboard(scenario.entry, scenario.env, (text) => {
  failure = text;
});
if (scenario.expectFailureCallback) {
  const deadline = Date.now() + 10_000;
  while (failure === null && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}
if (scenario.waitForFile) {
  const deadline = Date.now() + 10_000;
  while (!existsSync(scenario.waitForFile) && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  await new Promise((resolve) => setTimeout(resolve, 200));
}
void failed;
writeFileSync(resultFile, JSON.stringify({ message, failure }));
process.exit(0);
