import { Elysia, t } from "elysia";
import { getInscriptionsByPage } from "~/resources/inscriptions/service";

export const inscriptionsRouter = new Elysia({ prefix: "/inscriptions" }).get(
  "/",
  async ({ query }) => getInscriptionsByPage(query.pageIndex, query.pageSize),
  {
    query: t.Object({
      pageIndex: t.Numeric({ default: 0 }),
      pageSize: t.Numeric({ default: 20 }),
    }),
  }
);
