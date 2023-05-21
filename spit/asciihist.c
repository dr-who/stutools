#include "histogram.h"
#include "utils.h"



int main() {
  histogramType hist;
  histSetup(&hist, 0, 10, 0.1);
  /*  printf("%zd\n", hist.arraySize);
  printf("%lf\n", getIndexToXValue(&hist, 0));
  printf("%lf\n", getIndexToXValue(&hist, hist.arraySize));*/


  histAdd(&hist, 1);
  histAdd(&hist, 1.2);
  histAdd(&hist, 1.4);
  histAdd(&hist, 1);

  histSave(&hist, "aa");

  asciiField(&hist, 80, 20, "test");
  
  histFree(&hist);

  return 0;
}


