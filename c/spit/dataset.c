#include <stdlib.h>
#include <malloc.h>
#include <assert.h>
#include <string.h>
#include <math.h>

#include "dataset.h"


datasetType *datasetInit(void) {
  datasetType *res = calloc(sizeof(datasetType), 1); assert(res);
  return res;
}

void datasetFree(datasetType **din) {
  datasetType *d = *din;
  for (size_t i = 0; i < d->rows;i++) {
    for (size_t j = 0;j < d->columns; j++) {
      if (d->category[j] == LABEL) {
	free(d->data[i][j].v.label);
      }
    }
    free(d->data[i]);
  }
  free(d->data);
  
  for (size_t j = 0;j < d->columns; j++) {
    free(d->name[j]);
  }
  free(d->category);
  free(d->name);

    
  free(d);
  *din = NULL;
}

void datasetAddColumn(datasetType *d) {
  d->name = realloc(d->name, (d->columns + 1) * sizeof(char*)); assert(d->name);
  d->category = realloc(d->category, (d->columns + 1) * sizeof(enum thetype)); assert(d->category);
}
 

void datasetAddColumnLabel(datasetType *d, char *name) {
  datasetAddColumn(d);
  d->name[d->columns] = strdup(name);
  d->category[d->columns] = LABEL;
  d->columns++;
}

void datasetAddColumnNumber(datasetType *d, char *name) {
  datasetAddColumn(d);
  d->name[d->columns] = strdup(name);
  d->category[d->columns] = NUMBER;
  d->columns++;
}

void datasetAddColumnPrediction(datasetType *d, char *name) {
  for (size_t i = 0; i < d->columns ;i++) {
    if (d->category[i] == PREDICTION) { // already has a prediction
      fprintf(stderr,"*info* there is already a prediction column: %s\n", d->name[i]);
    }
  }
  datasetAddColumn(d);
  d->name[d->columns] = strdup(name);
  d->category[d->columns] = PREDICTION;
  d->columns++;
}
  

void datasetAddRow(datasetType *d, attrType *row) {
  d->data = realloc(d->data, ((d->rows + 1) * sizeof(attrType*)));
  assert (d->data);
  d->data[d->rows] = row;
  d->rows++;
}

//  datasetInitHeader(d, "l:country n:age\t n:amount \tn:items p:class\t\tp:spend2\n");
void datasetInitHeader(datasetType *d, char *headline) {
  char *dup = strdup(headline);
  char *tok = NULL;
  int first = 1;
  while ((tok = strtok(first ? dup : NULL, " \t,\r\n")) != NULL) {

    char *p = strchr(tok, ':');
    if (tok[0] == 'l') {
      datasetAddColumnLabel(d, p+1);
    } else if (tok[0] == 'n') {
      datasetAddColumnNumber(d, p+1);
    } else if (tok[0] == 'p') {
      datasetAddColumnPrediction(d, p+1);
    } else {
      fprintf(stderr,"not sure what to do with '%s'\n", tok);
    }
    first = 0;
  }
  free(dup);
}


void datasetAddDataLine(datasetType *d, char *line) {
  char *dup = strdup(line);
  int first = 1;
  char *tok = NULL;

  attrType *row = calloc(d->columns, sizeof(attrType)); assert(row);

  size_t column = 0;
  while ((tok = strtok(first ? dup : NULL, " \t,")) != NULL) {
    int nn =0;
    if (strcmp(tok, "?")==0) {
      nn = 1;
    }

    assert(column < d->columns);
    if (d->category[column] == LABEL) {
      row[column].v.label = nn ? NULL : strdup(tok);
    } else {
      row[column].v.number = nn ? NAN : atof(tok);
    }
    column++;
    first = 0;
  }
  datasetAddRow(d, row);
  free(dup);
}

//  datasetAddDataString(d, "usa 32 1 3 50\n");
void datasetAddDataString(datasetType *d, char *string) {
  if (d->columns <= 0) {
    fprintf(stderr, "A header is required first\n");
  } else {
    char *pos = string, *nl = NULL;
    while ((nl = strchr(pos, '\n')) != NULL) {
      char *row = calloc(nl - pos + 1+1, 1);
      snprintf(row, nl - pos+1, "%s", pos);

      datasetAddDataLine(d, row); // will parse and create
      pos += (nl - pos) + 1;
      free(row);
    }
  }
}
      


void datasetDumpJSON(datasetType *d) {
  printf("{ \n");
  for (size_t i = 0; i < d->columns ;i++) {
    printf("\t");
    printf("\"%s\": ", d->name[i]);
    if (d->category[i] == LABEL) {
      printf("\"%s\"", "Label");
    } else if (d->category[i] == PREDICTION) {
      printf("\"%s\"", "Prediction");
    } else if (d->category[i] == NUMBER) {
      printf("\"%s\"", "Number");
    }
    if (i < d->columns) {
      printf(",");
    }
    printf("\n");
  }
  printf("\t\"data\": [\n");
  for (size_t i = 0;i < d->rows; i++) {
    printf("{\n");
    for (size_t j = 0; j < d->columns ;j++) {
      printf("\t\"%s\": ", d->name[j]);
      if (d->category[j] == LABEL) {
	printf("\"%s\"", d->data[i][j].v.label ? d->data[i][j].v.label : "?");
      } else {
	if (isnan(d->data[i][j].v.number)) {
	  printf("\"?\"");
	} else {
	  printf("%lf", d->data[i][j].v.number);
	}
      }
      if (j < d->columns -1) {
	printf(",");
      }
      printf("\n");
    }
    printf("}");
    if (i < d->rows-1) {
      printf(",");
    }
    printf("\n");
  }
  printf("]\n");

  printf("}\n");
}
   

#include "numList.h"

void datasetStats(datasetType *d) {
  for (size_t i = 0; i < d->columns ;i++) {
    printf("[%zd] %s (%d)\n", i, d->name[i], d->category[i]);
    if (d->category[i] != LABEL) {
      numListType nl;
      nlInit(&nl, 100000);
      for (size_t j = 0; j < d->rows; j++) {
	//	fprintf(stderr,"rows %zd\n", j);
	//	fprintf(stderr,"  number %lf\n", d->data[j][i].v.number);
	if (!isnan(d->data[j][i].v.number)) {
	  nlAdd(&nl, d->data[j][i].v.number);
	}
      }
      nlSummary(&nl);
      nlFree(&nl);
    }
  }
}
