import { drizzle } from "drizzle-orm/postgres-js";
import postgres from "postgres";

const client = postgres({
  host: "postgres",
  port: 5432,
  db: process.env.POSTGRES_DB,
  user: process.env.POSTGRES_USER,
  pass: process.env.POSTGRES_PASSWORD,
});

export const db = drizzle(client);
