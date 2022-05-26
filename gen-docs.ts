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

interface MagicVar {
    name: string;
    desc: string;
    prop?: string;
}

function parseMagicVariables(data: string) {
    const desc_pattern = /^.*\/\/\s*@desc\s*(\w+)\s*-\s*(.*)$/; // matches `// @desc <name> - <desc>`
    const prop_pattern = /^.*\/\/\s+@prop +\s*(.*)$/; // matches `// @prop <prop>`
    const continuation = /^.*\/\/\s*(.*)$/; // matches `// <...>`
    let defs: MagicVar[] = [];
    let last: "desc" | "prop" | null = null; // what was the last line
    for (const line of data.split("\n")) {
        const desc = desc_pattern.exec(line);
        const prop = prop_pattern.exec(line);
        const cont = continuation.exec(line);
        if(desc) {
            defs.push({
                name: desc[1],
                desc: desc[2],
                prop: undefined,
            });
            last = "desc";
        } else if(prop && defs.length > 0) {
            defs[defs.length - 1].prop = prop[1];
            last = "prop";
        } else if(cont && defs.length > 0) {
            if(last == "desc") {
                defs[defs.length - 1].desc += `\n\n${cont[1]}`;
            } else if(last == "prop" && defs[defs.length - 1].prop) {
                defs[defs.length - 1].prop += `\n\n${cont[1]}`;
            } // else this is just a comment, we ignore
        } else {
            last = null;
        }
    }
    let output = "";
    for (const {name, desc, prop} of defs) {
        output +=
            `### \`${name}\`\n` +
            `${desc}\n`;
        if(prop != null) {
            output +=
                '#### Structure\n' +
                '```\n' +
                `${prop}\n` +
                '```\n';
        }
        output += '\n';
    }
    return output;
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
            const exts: string[] = newWidgetMatch[3]
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

        // if we find a property, check through the following lines until we reach the actual property definition
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
