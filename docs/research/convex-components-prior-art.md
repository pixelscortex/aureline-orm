# Convex Components — prior art for reusable, isolated backends

**Date:** 2026-08-15
**Researcher:** background research agent
**Question:** What exactly is a Convex Component, how are its schema, data, functions, dependencies, configuration, deployment, and transactions isolated, and where does the advertised sandbox stop?

This is a factual prior-art report, not an Aureline design or specification.

## Sources and version

Primary sources only were used:

- Convex documentation: [Components](https://docs.convex.dev/components), [Understanding Components](https://docs.convex.dev/components/understanding), [Using Components](https://docs.convex.dev/components/using), and [Authoring Components](https://docs.convex.dev/components/authoring).
- `get-convex/convex-js` at commit [`194859a`](https://github.com/get-convex/convex-js/tree/194859a579dda5b9646180816e24fd9009fbfbc5), especially the [component definition interface](https://github.com/get-convex/convex-js/blob/194859a579dda5b9646180816e24fd9009fbfbc5/src/server/components/index.ts), [CLI graph/bundling pipeline](https://github.com/get-convex/convex-js/blob/194859a579dda5b9646180816e24fd9009fbfbc5/src/cli/lib/components/definition/bundle.ts), and [component-aware push](https://github.com/get-convex/convex-js/blob/194859a579dda5b9646180816e24fd9009fbfbc5/src/cli/lib/components.ts).
- `get-convex/convex-backend` at commit [`c2a6b3d`](https://github.com/get-convex/convex-backend/tree/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866), especially [component tree deployment](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/model/src/components/config.rs), [reference resolution](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/model/src/components/mod.rs), [database syscall scoping](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/isolate/src/environment/udf/syscall.rs), [nested function execution](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/isolate/src/environment/udf/async_syscall.rs), and [auth propagation](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/model/src/components/auth.rs).
- The official component template at commit [`6d2d2c2`](https://github.com/get-convex/templates/tree/6d2d2c29195fd63c55c055a47527fd88d4f5f57e/template-component), including its [`package.json`](https://github.com/get-convex/templates/blob/6d2d2c29195fd63c55c055a47527fd88d4f5f57e/template-component/package.json), [client wrapper](https://github.com/get-convex/templates/blob/6d2d2c29195fd63c55c055a47527fd88d4f5f57e/template-component/src/client/index.ts), and [publishing guide](https://github.com/get-convex/templates/blob/6d2d2c29195fd63c55c055a47527fd88d4f5f57e/template-component/PUBLISHING.md).

The public TypeScript declarations still label Components as **beta** and explicitly say the interface is unstable ([`ComponentDefinition`, lines 87–109](https://github.com/get-convex/convex-js/blob/194859a579dda5b9646180816e24fd9009fbfbc5/src/server/components/index.ts#L87-L109)). Findings therefore describe this point in time.

## Executive finding

A Convex Component is best understood as a **reusable component definition that is instantiated inside one Convex deployment**. A definition contains a schema, functions, exported function references, declared environment variables, HTTP routes, and optionally child component definitions. `app.use(definition, { name, env, httpPrefix })` creates an installation. Installations form a rooted tree; the same definition can be installed multiple times under different names and each installation has its own functions and data ([Using Components, installation](https://docs.convex.dev/components/using#installation); [Authoring Components, anatomy](https://docs.convex.dev/components/authoring#anatomy-of-a-component)).

The isolation is substantive but specific:

- The backend assigns each installed component a `ComponentId` and uses it as a table/file/scheduler/function namespace.
- A function's database syscalls resolve table names only inside its current component namespace.
- Cross-component calls resolve through explicit exported references; ordinary component code has no ambient reference to its parent or siblings.
- Environment variables and user authentication do not implicitly cross into a child.
- Cross-component mutations execute in nested subtransactions within the caller's overall transaction.

It is **not** a separate database, process, deployment, operating-system sandbox, npm supply-chain sandbox, or general information-flow-control system. Component actions can perform network calls; data deliberately passed to a component can be returned or transmitted. Optional package “client” code runs in the parent app's environment. The phrase “sandbox” in the product documentation is therefore about Convex runtime resource access and execution-state separation, not every threat posed by third-party source packages.

## 1. Definition, installation, and identity

A component directory resembles a normal `convex/` directory:

```text
component/
├── _generated/
├── convex.config.ts
├── schema.ts
└── ... function modules
```

`defineComponent("name")` creates the definition; `component.use(child)` gives definitions dependencies; `defineApp()` creates the root. A definition may install another definition, so evaluation produces a **tree**, not a flat registry ([Authoring Components, anatomy](https://docs.convex.dev/components/authoring#anatomy-of-a-component)). The runtime object records the child installation name, the imported definition path, environment bindings, and optional HTTP prefix ([`use`, lines 416–475](https://github.com/get-convex/convex-js/blob/194859a579dda5b9646180816e24fd9009fbfbc5/src/server/components/index.ts#L416-L475)).

There are two different paths:

- A **component definition path** identifies source relative to the root `convex/` directory. The CLI uses this for discovery and bundling.
- A **component path** identifies an installed instance in the runtime tree, such as `workflow/workpool`.

Installation identity is structurally `(parent component, installation name)`. The backend loads existing instances by `parent_and_name` and modifies an existing node when that key remains present; a missing node is unmounted and a new key gets a fresh namespace ([tree diff and modify/create selection](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/model/src/components/config.rs#L429-L526)). Consequently, changing an npm package version while retaining the same tree position updates the existing installation and preserves its component identity; renaming/moving the installation is, by implementation inference, an unmount plus a new installation rather than an in-place rename.

## 2. Packaging and distribution

Definitions can come from a local folder or an npm package. The documented package entry points are:

- the ordinary package entry point for optional classes, helpers, constants, and parent-app wrappers;
- `/convex.config.js` for the component definition;
- `/_generated/component.js` for the outward-facing `ComponentApi` type;
- `/test` for `convex-test` registration helpers.

The official template publishes both `src` and compiled `dist`, generates the component's types with `convex codegen --component-dir`, compiles with TypeScript, and exposes the definition, outward type, client code, and test helper as separate package exports ([Authoring Components, build and entry points](https://docs.convex.dev/components/authoring#building-and-publishing-npm-package-components); [template `package.json`, lines 16–68](https://github.com/get-convex/templates/blob/6d2d2c29195fd63c55c055a47527fd88d4f5f57e/template-component/package.json#L16-L68)). Distribution versioning is ordinary npm semver; there is no separate component registry artifact or declared database-migration manifest in the component interface.

The optional package entry point deserves separate treatment from component backend code. Convex explicitly describes it as code that runs in the **app's environment**, where it can use `ctx.auth`, app environment variables, and other app resources; the docs say applications wanting tighter control may call component functions directly instead ([Authoring Components, wrapping client code](https://docs.convex.dev/components/authoring#wrapping-the-component-with-client-code)). The template client demonstrates reading `process.env`, constructing app-level query/mutation/action definitions, and accepting parent contexts ([template client](https://github.com/get-convex/templates/blob/6d2d2c29195fd63c55c055a47527fd88d4f5f57e/template-component/src/client/index.ts)). That code is an ordinary trusted library layer, not code confined by the component's runtime data namespace.

## 3. Discovery, analysis, code generation, and deployment

The CLI performs a whole-tree push:

1. esbuild discovers imports of `convex.config.*` and derives a dependency graph.
2. Each definition is bundled separately; imports of child definitions become external `_componentDeps/<base64-definition-path>` references.
3. Initial generated files make every definition bundleable and analyzable.
4. The CLI bundles every installation's schema and Convex-runtime function modules.
5. It uploads the root definition, all component definitions, implementations, dependency edges, schemas, and runtime version in one push request.
6. The server evaluates definitions, checks the component tree, starts schema/index validation, the CLI waits, and the server finishes the tree diff atomically.

The discovery and rewriting behavior is visible in the official [component esbuild plugin](https://github.com/get-convex/convex-js/blob/194859a579dda5b9646180816e24fd9009fbfbc5/src/cli/lib/components/definition/bundle.ts#L101-L233); definition and implementation bundles are deliberately separate ([bundle definitions](https://github.com/get-convex/convex-js/blob/194859a579dda5b9646180816e24fd9009fbfbc5/src/cli/lib/components/definition/bundle.ts#L467-L617), [bundle implementations](https://github.com/get-convex/convex-js/blob/194859a579dda5b9646180816e24fd9009fbfbc5/src/cli/lib/components/definition/bundle.ts#L619-L766)). The component-aware push then performs codegen, upload, schema waiting, and finish ([push orchestration](https://github.com/get-convex/convex-js/blob/194859a579dda5b9646180816e24fd9009fbfbc5/src/cli/lib/components.ts#L281-L530)).

Component definitions are evaluated as Convex isolate modules. They serialize a definition type, child instantiations, export tree, HTTP mounts, and environment-variable validators; the public builder is configuration code producing this metadata, not a general install script ([definition serialization](https://github.com/get-convex/convex-js/blob/194859a579dda5b9646180816e24fd9009fbfbc5/src/server/components/index.ts#L492-L646)).

One current implementation restriction is explicit: Node-runtime (`"use node"`) modules are rejected in non-root component directories; component implementations are bundled for the Convex isolate runtime ([bundler rejection](https://github.com/get-convex/convex-js/blob/194859a579dda5b9646180816e24fd9009fbfbc5/src/cli/lib/components/definition/bundle.ts#L702-L754)).

## 4. Schema and storage isolation

Each installed instance receives a distinct `ComponentId`. For a newly created child, deployment initializes component-local system tables and applies its schema in `TableNamespace::from(component_id)` ([namespace allocation and schema submission](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/model/src/components/config.rs#L265-L341)). Two instances of the same definition therefore share implementation source but do not share table mappings or documents.

This is enforced below the TypeScript interface. A database syscall obtains the executing function's component from the phase, then performs table lookup and query compilation against only that namespaced mapping ([database syscall scoping, lines 61–104](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/isolate/src/environment/udf/syscall.rs#L61-L104)). Storage and scheduler operations similarly carry the executing component identity; the public documentation describes file storage and scheduled functions as independent per component ([Understanding Components, Data](https://docs.convex.dev/components/understanding#data)).

IDs expose an important boundary cost. An `Id<"users">` inside one component is not the same table identity as `Id<"users">` elsewhere. Convex currently converts IDs to plain strings in the outward `ComponentApi`, and a validator cannot name a table in another component ([Authoring Components, IDs](https://docs.convex.dev/components/authoring#id-types-and-validation)). This prevents the outward type from falsely claiming that two same-named tables share an ID domain, but loses table-specific static typing at the component boundary.

The main app cannot directly query or mutate a child's tables through its own `ctx.db`; it must call an exported function. Conversely, child database syscalls cannot resolve the root's table names. This is stronger than a naming convention because the component ID chooses the backend table mapping before a developer table name is resolved.

It is nevertheless one deployment and one transactional database. The implementation uses logical namespaces and component-scoped system tables, not separate database servers.

## 5. Function exports, references, handles, and calls

Each component gets generated `api`, `internal`, `components`, `server`, `dataModel`, and `component` views specific to its own definition. From the parent, `components.foo` exposes only functions declared public by `foo`; those references are converted to internal visibility, so they can be called by server functions but are not automatically reachable by clients over HTTP/WebSockets ([Authoring Components, Component API](https://docs.convex.dev/components/authoring#the-component-api)). An app must deliberately wrap or re-export them to create a public app API.

Runtime references are relative capabilities:

- `api.x` resolves a function in the current component.
- `components.child.x` resolves a public export of a direct child.
- Nested children are reached only through exports selected by each intermediate definition.

The backend resolver looks up a named child under the current component, resolves only a public exported function, and fails if the export is absent ([reference resolution](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/model/src/components/mod.rs#L43-L144)). Its preloaded resource set consists of the component's own functions plus its children's exported resources, not arbitrary paths to the parent or siblings ([resource preload](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/model/src/components/mod.rs#L244-L304)).

For deliberate callbacks, `createFunctionHandle(reference)` converts an already-valid function reference into a serializable string. The string can cross function boundaries, be stored, invoked, or scheduled. It is stable across code pushes, although the referenced function can later cease to exist; argument and return validators still run ([Function Handles](https://docs.convex.dev/components/authoring#function-handles); [`FunctionHandle` declaration](https://github.com/get-convex/convex-js/blob/194859a579dda5b9646180816e24fd9009fbfbc5/src/server/components/index.ts#L25-L81)). A handle is therefore an explicit escape from the parent→child-only static reference graph, not ambient discovery by the child.

Argument and return validators are runtime-enforced at the boundary. The nested executor validates arguments before execution and validates the result using the callee's table mapping afterwards ([nested call validation](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/isolate/src/environment/udf/async_syscall.rs#L452-L476), [return validation](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/isolate/src/environment/udf/async_syscall.rs#L605-L615)). Omitting public validators degrades the generated TypeScript surface to `any`, so end-to-end typing is conditional on component authors declaring both sides ([Authoring Components, validation](https://docs.convex.dev/components/authoring#validation)).

Normal function-kind rules remain intact: a query may call a query, a mutation may call queries or mutations, and actions may call actions in addition to query/mutation calls ([Using Components, direct API](https://docs.convex.dev/components/using#using-the-components-api-directly)).

## 6. Transaction behavior

Cross-component mutation calls do not become independent distributed transactions. All successful writes remain part of the top-level mutation and commit together. If the top-level mutation fails, writes in every called component roll back ([Using Components, Transactions](https://docs.convex.dev/components/using#transactions)).

Each nested mutation call additionally receives a subtransaction/savepoint. The executor begins a subtransaction before the call, rolls it back when the nested mutation returns an error, and otherwise commits it into the still-uncommitted parent transaction ([nested executor, lines 489–559](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/isolate/src/environment/udf/async_syscall.rs#L489-L559)). Therefore:

- an uncaught component exception aborts the caller and ultimately the whole top-level mutation;
- a caught component exception discards only that component call's tentative changes, after which the caller may continue;
- “component transaction isolation” means rollback isolation inside one encompassing transaction, not independent commits.

This is a particularly strong composability guarantee, but it also couples component calls to Convex's single database transaction implementation.

## 7. Environment variables, authentication, and HTTP routes

Environment access is explicit. A definition declares string-like validators for environment variables; required variables make the `env` installation option type-required. A parent supplies literals or binds a child variable by reference to one of its own declared variables ([typed installation options](https://github.com/get-convex/convex-js/blob/194859a579dda5b9646180816e24fd9009fbfbc5/src/server/components/index.ts#L139-L189), [Authoring Components, environment variables](https://docs.convex.dev/components/authoring#environment-variables)). Undeclared app variables are not present inside the component; only Convex's system URLs are ambient. Declared values are available through generated typed `env` and through `process.env` without the stronger typing.

User authentication is also intentionally not inherited. Public documentation says `ctx.auth` is unavailable inside components and recommends authenticating in a root wrapper, then passing a user/application identifier explicitly ([Authoring Components, authentication](https://docs.convex.dev/components/authoring#authentication-via-ctxauth)). The backend implements this by preserving ordinary user identity only for root→root calls; calls crossing a component boundary receive an unknown identity, with an internal exception for administrator identity ([auth propagation](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/model/src/components/auth.rs#L1-L19)). The admin exception is implementation detail, not an advertised application authentication mechanism.

Component HTTP actions are inert until the installing parent assigns an `httpPrefix`; otherwise no component route is exposed. A mounted route is namespaced under that prefix. Component HTTP actions still lack app auth and app environment variables, so the documented pattern for those needs is an app-owned HTTP handler calling the component ([Using Components, HTTP Routes](https://docs.convex.dev/components/using#http-routes); [Authoring Components, HTTP Actions](https://docs.convex.dev/components/authoring#http-actions)).

## 8. Update, unmount, schema change, and migration behavior

There is no per-component SQL migration stream or package install hook in the public component definition. A component update ships a new desired schema and code as part of the whole deployment push. Convex submits schema validation and index changes independently for each component namespace, waits for them, and then applies the tree/code diff.

Schema evolution follows ordinary Convex document-schema rules. Data transformations are functions, usually performed through the separate Migrations Component; the official docs describe it as an online, resumable, dry-runnable mechanism for changing live documents ([Writing Data, Migrations](https://docs.convex.dev/database/writing-data#migrations)). This is distinct from the mechanism that installs or upgrades a component package.

Removal is intentionally two-stage in the backend:

1. Removing an installation from the desired tree **unmounts** it. The backend removes modules, cron definitions, and function handles and marks the component inactive.
2. Its existing schema and tables are deliberately left in place, allowing a later installation at the same tree position to **remount** with the same component identity and retained data.
3. A separate destructive delete requires the component to be unmounted, then deletes its schema table and every table in its namespace.

The preservation behavior is explicit in [`start_component_schema_changes`](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/model/src/components/config.rs#L265-L351); unmount/remount is implemented in [`apply_component_tree_diff` and `unmount_component`](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/model/src/components/config.rs#L429-L526) and [the destructive delete path](https://github.com/get-convex/convex-backend/blob/c2a6b3dc05271ba3f3ca8e7ffbf35029681de866/crates/model/src/components/config.rs#L676-L765). This lifecycle is not prominently documented on the public Components pages, so callers should not infer it solely from the word “remove.”

## 9. Nesting and dependency consequences

Nesting is transitive but access is local. A component author chooses its children and their names/configuration. An app installing that parent receives the parent's exported interface, not automatic access to every descendant. A descendant likewise has no implicit route back to an ancestor. This is enforced through the reference graph described above.

Multiple installations of one definition are first-class and independently stateful. Names are therefore instance names, not merely import aliases. Paths also appear in the CLI (`--component workflow/workpool`), dashboard filtering, and logs (`component_path`) ([Using Components, Dashboard and logs](https://docs.convex.dev/components/using#dashboard)).

The dependency graph is resolved from source imports during the consuming application's build. It is not a remote service discovery graph: all participating component definitions and implementations are bundled into the application's push, evaluated, typechecked, and deployed together.

## 10. Current limitations visible in public docs/source

- Components remain a beta/unstable programming interface.
- Backend functions and component definitions are TypeScript/JavaScript; this is not a language-neutral component ABI.
- Node-runtime actions are rejected inside components at this source revision.
- Component table IDs degrade to strings at an outward boundary; cross-component `v.id(table)` validators are unsupported.
- Built-in `.paginate()` does not work in components; the docs prescribe helpers with different client behavior ([Authoring Components, Pagination](https://docs.convex.dev/components/authoring#pagination)).
- `ctx.auth` is unavailable inside component functions and HTTP actions.
- A component's public functions are not automatically public application endpoints; wrappers or explicit re-export are required.
- HTTP routes require an app-selected mount prefix.
- Function handles are stable identifiers but can become dangling after code changes.
- Static TypeScript safety depends on argument/return validators; missing validators yield `any`.
- The public interface contains no component package version constraint, upgrade hook, downgrade protocol, compatibility declaration, or migration manifest beyond npm versioning and the desired deployed schema/code.

## 11. What “sandboxed” means—and does not mean

| Concern | Mechanism found | Classification |
|---|---|---|
| Direct database access | Backend chooses a table mapping from current `ComponentId` before resolving table names | Enforced isolation |
| File storage and scheduled jobs | Component identity is carried into resource operations; docs expose separate component views | Enforced namespace/isolation |
| Calling other backend code | Own functions plus public exports of declared children; callback handles must be explicitly passed | Enforced reference graph with explicit delegation |
| Arguments and return values | Runtime validators checked across calls | Enforced contract when validators exist |
| User identity | Stripped across component calls; app passes identifiers explicitly | Enforced isolation |
| Environment variables | Only declared/bound values plus Convex system URLs | Enforced isolation |
| Global JavaScript mutations | Docs guarantee execution environments are separated between components | Runtime-state isolation |
| Transaction commit | Same top-level transaction, nested rollback savepoints | Transactional composition, not isolation by separate database |
| External network | Component isolate actions can use `fetch` | Not information-flow confinement |
| Values explicitly passed in | Component can compute with, return, store, or transmit them | Explicit trust/delegation, not leak prevention |
| npm install/build | Ordinary npm package and local build/codegen workflow | No package-install sandbox established by reviewed sources |
| Optional package client code | Runs in app environment and may receive app contexts/auth/env | Ordinary trusted library code |
| Physical tenancy/process | Components share one Convex deployment and database implementation | Logical/runtime isolation, not separate infrastructure |

The most precise reading of Convex's guarantee is: **component backend functions lack ambient authority to another component's Convex resources**. That statement is supported by both the documentation and backend source. Stronger claims—such as “third-party packages cannot execute locally,” “a component cannot send explicitly provided data over the network,” or “each component is a separate OS security principal”—are not supported by the reviewed mechanisms.

## 12. Compact mechanism summary

```text
npm/local component definition
    │  schema + function modules + convex.config.ts + outward ComponentApi type
    ▼
CLI discovers a definition DAG and bundles each definition/implementation
    ▼
server evaluates definitions and checks a rooted installation tree
    ▼
each installed tree node gets ComponentId + namespaced resources
    │
    ├── database / files / scheduler resolved in that namespace
    ├── own functions
    ├── public exports of declared children
    ├── explicitly bound env vars
    └── no inherited application user auth
    ▼
cross-component call
    ├── validate args
    ├── execute in callee namespace
    ├── nested mutation savepoint
    └── validate return
```

That combination—package distribution, generated outward type, backend-enforced per-instance resource namespace, explicit exported function graph, and same-database subtransactions—is the factual core of Convex Components.
