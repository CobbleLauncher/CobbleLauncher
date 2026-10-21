// SPDX-License-Identifier: MIT
/** SPDX expression-aware policy for the actually installed pnpm graph. */
import { execFileSync } from 'node:child_process';
import { readFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import path from 'node:path';
import parse from 'spdx-expression-parse';

export function allowedExpression(expression, allowed) {
  try {
    const check = (node) => {
      if (node.conjunction) {
        return node.conjunction === 'or'
          ? check(node.left) || check(node.right)
          : check(node.left) && check(node.right);
      }
      const identifier = `${node.license}${node.plus ? '+' : ''}${node.exception ? ` WITH ${node.exception}` : ''}`;
      return allowed.has(identifier);
    };
    return check(parse(expression));
  } catch {
    return false;
  }
}

export function developmentException(entry, productionKeys, exceptions) {
  if (productionKeys.has(`${entry.name}@${entry.version}`)) return null;
  return exceptions.find((rule) => {
    const matchesName = rule.name.endsWith('-*')
      ? entry.name.startsWith(rule.name.slice(0, -1))
      : rule.name === entry.name;
    return matchesName && rule.version === entry.version && rule.license === entry.license;
  }) ?? null;
}

function main() {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  const policy = JSON.parse(readFileSync(path.join(root, 'scripts/license-policy.json'), 'utf8'));
  // Invoke pnpm's actual JS entry with Node, never a shell string or .cmd shim.
  const pnpmScript = process.env.npm_execpath;
  if (!pnpmScript) throw new Error('Run this check through pnpm licenses:check');
  const grouped = JSON.parse(execFileSync(process.execPath, [pnpmScript, 'licenses', 'list', '--json'], {
    cwd: root, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024,
  }));
  const production = JSON.parse(execFileSync(process.execPath, [pnpmScript, 'licenses', 'list', '--prod', '--json'], {
    cwd: root, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024,
  }));
  const productionKeys = new Set(Object.values(production).flatMap((entries) =>
    entries.flatMap((entry) => entry.versions.map((version) => `${entry.name}@${version}`))));
  const allowed = new Set(policy.allowed);
  const packages = [];
  for (const entries of Object.values(grouped)) {
    for (const entry of entries) {
      for (const version of entry.versions) {
        const candidate = { name: entry.name, version, license: entry.license || 'NOASSERTION' };
        const exception = developmentException(candidate, productionKeys, policy.developmentOnlyExceptions);
        packages.push({
          name: entry.name,
          version,
          license: entry.license || 'NOASSERTION',
          allowed: allowedExpression(entry.license, allowed) || exception !== null,
          productionDependency: productionKeys.has(`${entry.name}@${version}`),
          exceptionReason: exception?.reason ?? null,
          source: `https://registry.npmjs.org/${entry.name}/${version}`,
        });
      }
    }
  }
  packages.sort((a, b) => `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`, 'en'));
  const rejected = packages.filter((entry) => !entry.allowed);
  const directory = path.join(root, 'validation/dependencies');
  mkdirSync(directory, { recursive: true });
  writeFileSync(path.join(directory, 'node-licenses.json'), JSON.stringify({
    scope: 'Installed pnpm packages on this platform, including development dependencies; not bundled browser/OS/WebView components.',
    packages,
    rejected,
  }, null, 2) + '\n');
  console.log(`Node license policy: ${packages.length} package versions, ${rejected.length} requiring review.`);
  if (rejected.length) {
    console.error(JSON.stringify(rejected, null, 2));
    process.exitCode = 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
