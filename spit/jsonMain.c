#include "json.h"

int main(void) {
  jsonElements *j = jsonInit();

  jsonValue *o = jsonValueObject();

  jsonValue *nodes = jsonValueArray();
  
  jsonValue *node = jsonValueObject();
  jsonObjectAdd(node, "node", jsonValueString("node1"));
  jsonObjectAdd(node, "CPU", jsonValueString("Intel"));
  jsonObjectAdd(node, "RAM", jsonValueNumber(8));

  jsonArrayAdd(nodes, node);

  jsonValue *node2 = jsonValueObject();
  jsonObjectAdd(node2, "node", jsonValueString("node2"));
  jsonObjectAdd(node2, "CPU", jsonValueString("AMD/Intel"));
  jsonObjectAdd(node2, "RAM", jsonValueNumber(11));

  jsonArrayAdd(nodes, node2);

  jsonObjectAdd(o, "nodes", nodes);
  jsonElementAdd(j, o);

  
  jsonElementsDump(j, 1);


  return 0;
}

