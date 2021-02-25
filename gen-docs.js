const fs = require("fs");
// magic vars
fs.appendFileSync(
    "./docs/content/main/magic-vars.md",
    parseMagicVariables(fs.readFileSync("./src/config/inbuilt.rs", "utf8")) +
        "\n## Static Magic Variables\n\n" +
        parseMagicVariables(fs.readFileSync("./src/app.rs", "utf8"))
);
// Wigdet docs
let data1 = fs.readFileSync("./src/widgets/widget_definitions.rs", "utf8");
writeDocs(parseVars(data1), parseDocs(data1));

function parseMagicVariables(data) {
    const pattern = /^.*\/\/ @desc +(.*)$/;
    let output = "";
    for (let line of data.split("\n")) {
        let match = line.match(pattern);
        if (match) {
            let split = match[1].split("-");
            let name = split[0].trim();
            let desc = split[1].trim();
            output = output + `### \`${name}\`\n${desc}\n`;
        }
    }
    return output;
}

function parseVars(code) {
    const VAR_PATTERN = /^.*\/\/+ *@var +(.*?) +- +(.*)$/;
    const vars = {};
    for (let line of code.split("\n")) {
        const match = line.match(VAR_PATTERN);
        if (match && match.length == 3) {
            const name = match[1];
            const value = match[2];
            vars[name] = value;
        }
    }
    return vars;
}

function parseDocs(code) {
    const NEW_WIDGET_PATTERN = /^.*\/\/+ *@widget +(!?)(.*?)(?: +extends +(.*))?$/;
    const DESC_PATTERN = /^.*\/\/+ *@desc +(.*)$/;
    const PROP_PATTERN = /^.*\/\/+ *@prop +(.*?) +- +(.*)$/;

    const widgets = {};
    let currentWidget = "";
    for (let line of code.split("\n")) {
        const newWidgetMatch = line.match(NEW_WIDGET_PATTERN);
        if (newWidgetMatch && newWidgetMatch.length >= 3) {
            const name = newWidgetMatch[2];
            /** @type string[] */
            const exts = newWidgetMatch[3]
                ? newWidgetMatch[3].split(/, */)
                : [];
            currentWidget = name;
            widgets[currentWidget] = {
                name,
                exts,
                props: [],
                isVisible: newWidgetMatch[1] !== "!",
            };
            continue;
        }

        const descMatch = line.match(DESC_PATTERN);
        if (descMatch && descMatch.length == 2) {
            widgets[currentWidget].desc = descMatch[1];
            continue;
        }

        const propMatch = line.match(PROP_PATTERN);
        if (propMatch && propMatch.length == 3) {
            widgets[currentWidget].props.push({
                name: propMatch[1],
                desc: propMatch[2],
            });
        }
    }
    return widgets;
}

function writeDocs(vars, docs) {
    let output = Object.values(docs)
        .filter((x) => x.isVisible)
        .map((x) => {
            x.props = [
                ...x.props,
                ...x.exts.map((w) => docs[w]).flatMap((w) => w.props),
            ];
            return x;
        })
        .map((x) => returnWidget(x))
        .map((x) => x.replace(/\$\w+/g, (x) => vars[x.replace("$", "")]))
        .join("\n\n");
    fs.appendFileSync("./docs/content/main/widgets.md", output);
}

function returnWidget(widget) {
    return `
## \`${widget.name}\` ${widget.desc ? `\n${widget.desc}` : ""}

**Properties**
${widget.props.map((prop) => `- **\`${prop.name}\`**: ${prop.desc}`).join("\n")}
`;
}

// vim:ft=javascript
