import Datastore from '@seald-io/nedb';

export type Db = {
  documents: Datastore;
  chunks: Datastore;
  jobPostings: Datastore;
  jobInteractions: Datastore;
  conversations: Datastore;
  messages: Datastore;
  jobs: Datastore;
  matches: Datastore;
  autopilots: Datastore;
};

export function createDb(file: string): { db: Db } {
  const documents = new Datastore({ filename: `${file}.documents`, autoload: true });
  const chunks = new Datastore({ filename: `${file}.chunks`, autoload: true });
  const jobPostings = new Datastore({ filename: `${file}.job_postings`, autoload: true });
  const jobInteractions = new Datastore({ filename: `${file}.job_interactions`, autoload: true });
  const conversations = new Datastore({ filename: `${file}.conversations`, autoload: true });
  const messages = new Datastore({ filename: `${file}.messages`, autoload: true });
  const jobs = new Datastore({ filename: `${file}.jobs`, autoload: true });
  const matches = new Datastore({ filename: `${file}.matches`, autoload: true });
  const autopilots = new Datastore({ filename: `${file}.autopilots`, autoload: true });

  // Create indexes for better query performance
  chunks.ensureIndex({ fieldName: 'documentId' });
  jobPostings.ensureIndex({ fieldName: 'source' });
  jobPostings.ensureIndex({ fieldName: 'externalId' });
  jobInteractions.ensureIndex({ fieldName: 'jobId' });
  jobInteractions.ensureIndex({ fieldName: 'interactionType' });
  messages.ensureIndex({ fieldName: 'conversationId' });

  const db: Db = {
    documents,
    chunks,
    jobPostings,
    jobInteractions,
    conversations,
    messages,
    jobs,
    matches,
    autopilots,
  };

  return { db };
}
