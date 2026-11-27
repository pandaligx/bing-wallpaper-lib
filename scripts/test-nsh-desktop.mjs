// Run against a disposable graphical CI session. This test changes its desktop background.
await app.waitFor("latest-photo", {timeout: 30000});
await app.setViewport(1100, 740);
await app.setColorScheme("light");
await new Promise(resolve => setTimeout(resolve, 1000));
await app.screenshot("target/nsh-validation/light.png");
await app.press("latest-photo");
await app.waitFor("apply-photo");
await app.setColorScheme("dark");
await new Promise(resolve => setTimeout(resolve, 750));
await app.screenshot("target/nsh-validation/dark-preview.png");
const previewDate = () => app.texts().find(text => /^\d{4}-\d{2}-\d{2} ·/.test(text));
const first = previewDate();
if (!first) throw Error("Preview date missing");
await app.keyboard.press("right");
await app.waitUntil(() => !!previewDate() && previewDate() !== first);
await app.keyboard.press("left");
await app.waitFor(first);
await app.keyboard.press("escape");
await app.waitFor("apply-photo", {state: "gone"});
// Wait for the native modal's exit animation before sending Return to the original card.
await new Promise(resolve => setTimeout(resolve, 500));
await app.keyboard.press("enter");
await app.waitFor("apply-photo");
await app.press("apply-photo");
await app.press("close-preview");
await app.waitFor("applied", {timeout: 120000});
