import 'dotenv/config';
import { startServer } from './app.js';

const port = parseInt(process.env.PORT || '5000', 10);

startServer(port);
