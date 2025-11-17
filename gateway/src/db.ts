import pgPromise from 'pg-promise';
import { IDatabase, IMain } from 'pg-promise';

const pgp: IMain = pgPromise({
  capSQL: true,
});

const db: IDatabase<any> = pgp(process.env.DATABASE_URL || '');

export { db };
export default db;
