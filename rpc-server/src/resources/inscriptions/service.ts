import { desc } from "drizzle-orm";
import { db } from "~/database";
import { inscriptions } from "~/database/schema";

export async function getInscriptionsByPage(pageIndex = 0, pageSize = 20) {
  return await db
    .select()
    .from(inscriptions)
    .orderBy(desc(inscriptions.updated_on))
    .limit(pageSize)
    .offset(pageIndex * pageSize);
}
