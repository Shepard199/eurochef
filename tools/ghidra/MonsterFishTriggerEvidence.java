import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.DataIterator;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;

public class MonsterFishTriggerEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  DataIterator it=currentProgram.getListing().getDefinedData(true);
  while(it.hasNext()){
   Data d=it.next();
   Object v=d.getValue();
   if(v==null || !String.valueOf(v).contains("XTrigger_Monster_Fish")) continue;
   println("STRING "+d.getAddress()+" = "+v);
   for(Reference ref:getReferencesTo(d.getAddress())){
    Function f=getFunctionContaining(ref.getFromAddress());
    println(" REF "+ref.getFromAddress()+" "+ref.getReferenceType()+" "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
   }
  }
 }
}