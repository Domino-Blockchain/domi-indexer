import { bigint, customType, pgTable, text, timestamp } from "drizzle-orm/pg-core";

const bytea = customType<{ data: string; notNull: false; default: false }>({
  dataType() {
    return "bytea";
  },
  toDriver(val) {
    let newVal = val;
    if (val.startsWith("0x")) {
      newVal = val.slice(2);
    }
    return Buffer.from(newVal, "utf-8");
  },
  fromDriver(val) {
    if (val instanceof Buffer) {
      return val.toString("utf-8");
    }
    return "";
  },
});

export const inscriptions = pgTable("inscriptions", {
  slot: bigint("slot", { mode: "number" }).notNull(),
  signature: text("signature").notNull(),
  account: text("account").notNull(),
  metadata_account: text("metadata_account").notNull(),
  authority: text("authority").notNull(),
  data: bytea("data"),
  write_version: bigint("write_version", { mode: "number" }),
  updated_on: timestamp("updated_on").notNull(),
});
