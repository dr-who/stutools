#ifndef __DATASET_H
#define __DATASET_H

#include <unistd.h>

enum thetype {
  LABEL, // unique like name or IP
  ENUM,   // string class
  NUMBER,
  ZNUMBER,
  DERIVED,
  PREDICTION
};
    
typedef struct {
  union {
    char *label;   // NULL means missing
    double number; // NAN means missing
  } v;
  double mean;
  double sd;
} attrType;

typedef struct {
  size_t columns;
  char **name;
  enum thetype *category;

  size_t rows;
  attrType **data;
} datasetType;

datasetType * datasetInit(void);

void datasetAddColumnLabel(datasetType *d, char *name);
void datasetAddColumnNumber(datasetType *d, char *name);
void datasetAddColumnPrediction(datasetType *d, char *name);
void datasetFree(datasetType **din);

void datasetDumpJSON(datasetType *d);
void datasetAddRow(datasetType *d, attrType *row);

void datasetInitHeader(datasetType *d, char *headerline);
void datasetAddDataString(datasetType *d, char *string);

void datasetStats(datasetType *d);

#endif
