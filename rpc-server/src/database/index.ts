import { drizzle } from "drizzle-orm/postgres-js";
import postgres from "postgres";

const config = await Bun.file("../config.json").json();
const connectionString = config.connection_str as string;

const client = postgres(
  connectionString
    .split(" ")
    .reduce(
      (acc, pair) => ({ ...acc, ...Object.fromEntries([pair.split("=")]) }),
      {} as Record<string, string>
    )
);

export const db = drizzle(client);
