import assert from "node:assert/strict";
import test from "node:test";

import { createSchema } from "./index.js";

test("createSchema keeps the schema name and model list", () => {
  const schema = createSchema("blog", [{ name: "Post", fields: [] }]);

  assert.equal(schema.name, "blog");
  assert.equal(schema.models.length, 1);
  assert.equal(schema.models[0]?.name, "Post");
});
