interface WidgetData {
    name: string;
    desc: string;
    type: string; // this should be an enum.. maybe
}

interface Widget {
    name: string;
    exts: string[];
    desc: string;
    props: WidgetData[];
    isVisible: boolean;
}

function parseMagicVariables(data: string) {
    const pattern = /^.*\/\/\s*@desc\s*(\w+)\s*-\s*(.*)$/gm;
    const prop_pattern = /^.*\/\/\s+@prop +\s*(.*)$/gm;
    let properties = [...data.matchAll(prop_pattern)]
    let output = [];
    let i = 0;
    for (const [_, name, desc] of data.matchAll(pattern)) {
        output.push(
`### \`${name}\`
${desc.replaceAll("\\n", "\n\n")}
#### Structure
\`\`\`
${properties[i][1]}
\`\`\`
`);
        i = i + 1
    }
    return output.join("\n");
}

function parseVars(code: string): Record<string, string> {
    const VAR_PATTERN = /^.*\/\/+ *@var +(.*?) +- +(.*)$/;
    const vars: Record<string, string> = {};

    for (const line of code.split("\n")) {

        const match = line.match(VAR_PATTERN);
        if (match && match.length == 3) {
            const name = match[1];
            const value = match[2];
            vars[name] = value;
        }
    }
    return vars;
}

function replaceTypeNames(type: string) {

    switch (type) {
        case "f64":
        case "f32":
            return "float"
        case "i32":
        case "i64":
            return "int"
        default:
            return type
    }

}

function parseDocs(code: string) {
    const NEW_WIDGET_PATTERN = /^.*\/\/+ *@widget +(!?)(.*?)(?: +extends +(.*))?$/;
    const DESC_PATTERN = /^.*\/\/+ *@desc +(.*)$/;
    const PROP_PATTERN = /^.*\/\/+ *@prop +(.*?) +- +(.*)$/;
    const ARG_TYPE_PATTERN = /(\w+):\s+as_(\w+)/g;

    const widgets: Record<string, Widget> = {};
    const lines = code.split("\n")

    let currentWidget = "";

    for (let lineIndex = 0; lineIndex < lines.length; lineIndex++) {

        const line = lines[lineIndex]
        const newWidgetMatch = NEW_WIDGET_PATTERN.exec(line);

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
                desc: "",
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
            let no = lineIndex + 1

            while (/\s*\/\//.test(lines[no])) {
                no += 1
            } // continue till you find the line with actual code

            const matches = [...lines[no].matchAll(ARG_TYPE_PATTERN)].map(z => { z.shift(); return z }).flat() // try to use the iterator directly

            const possibleMatch = matches.findIndex(x => x == propMatch[1].replaceAll("-", "_"))
            if (possibleMatch == -1) {
                console.log(`Failed to find a match for "${propMatch[1].replace("-", "_")}" ~ ${JSON.stringify(matches, null, 2)} ~ ${lines[no]}`)
            }

            if (!widgets[currentWidget].props.some(p => p.name == propMatch[1])) {
                const type = replaceTypeNames(matches[possibleMatch + 1])

                widgets[currentWidget].props.push({
                    name: propMatch[1],
                    desc: propMatch[2],
                    type: type ?? "no-type-found"
                });
            }
        }
    }
    return widgets;
}

function printDocs(vars: Record<string, string>, docs: Record<string, Widget>) {
    const output = Object.values(docs)
        .filter((x) => x.isVisible)
        .map((x) => {
            x.props = [
                ...x.props,
                ...x.exts.map((w) => docs[w]).flatMap((w) => w.props),
            ];
            return x;
        })
        .map((x) => printWidget(x))
        .map((x) => x.replace(/\$\w+/g, (x) => vars[x.replace("$", "")]))
        .join("\n\n");
    return output;
}

function printWidget(widget: Widget) {
    return `
## \`${widget.name}\` ${widget.desc ? `\n${widget.desc}` : ""}

**Properties**
${widget.props.map((prop) => `- **\`${prop.name}\`**: *\`${prop.type}\`* ${prop.desc}`).join("\n")}
`;
}

// Deno args start from actual args
// Redirect stderr to ignore deno checking messages so:
// deno run --allow-read gen-docs.ts ./src/widgets/widget_definitions.ts 2> /dev/null
Deno.readTextFile(Deno.args[0]).then(data => {
    const vars = parseVars(data);
    Deno.writeTextFile("./docs/src/widgets.md", printDocs(vars, parseDocs(data)), {"append": true});
}).catch(err => {
    return console.error(err);
})

let magic = Deno.readTextFile(Deno.args[1]).then(data => {
    Deno.writeTextFile("./docs/src/magic-vars.md", parseMagicVariables(data), {"append": true});
}).catch(err => {
    return console.error(err);
})
