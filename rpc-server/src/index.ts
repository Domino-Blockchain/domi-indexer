import { Elysia } from "elysia";
import { inscriptionsRouter } from "~/resources/inscriptions";

const app = new Elysia().use(inscriptionsRouter).listen(process.env.PORT ?? 4000);

console.log(`🦊 Indexer RPC is running at ${app.server?.hostname}:${app.server?.port}`);
