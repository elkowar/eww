const fs = require("fs");
fs.readFile(process.argv[process.argv.length - 1], "utf8", (err, data) => {
    if (err) {
        return console.log(err);
    } else {
        const vars = parseVars(data);
        printDocs(vars, parseDocs(data));
    }
});

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

function printDocs(vars, docs) {
    let output = Object.values(docs)
        .filter((x) => x.isVisible)
        .map((x) => {
            x.props = [
                ...x.props,
                ...x.exts.map((w) => docs[w]).flatMap((w) => w.props),
            ];
            return x;
        })
        .map((x) => printWidget(x))
        .map((x) => x.replace(/\$\w+/, (x) => vars[x.replace("$", "")]))
        .join("\n\n");
    let md = `
+++
title = "Widgets"
slug = "Documentation of all available widgets and all their attributes"
weight = 4
+++

# Widgets

${output}
    `;
    console.log(md);
}

function printWidget(widget) {
    return `
## ${widget.name} ${widget.desc ? `\n${widget.desc}` : ""}

**Properties**
${widget.props.map((prop) => `- **${prop.name}**: ${prop.desc}`).join("\n")}
`;
}

// vim:ft=javascript
