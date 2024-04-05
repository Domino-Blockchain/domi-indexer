import { cors } from "@elysiajs/cors";
import { Elysia } from "elysia";
import { inscriptionsRouter } from "~/resources/inscriptions";

const app = new Elysia()
  .use(cors())
  .use(inscriptionsRouter)
  .listen(process.env.PORT ?? 4000);

console.log(`🦊 Indexer RPC is running at ${app.server?.hostname}:${app.server?.port}`);
