import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.symbol.Reference;
public class PiranhaHeightCallsiteEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  Address s=toAddr(0x00467A20L); disassemble(s);
  InstructionIterator it=currentProgram.getListing().getInstructions(s,true);
  int n=0;
  while(it.hasNext()&&n<120){
   Instruction ins=it.next();
   if(ins.getAddress().getOffset()>=0x00467B50L) break;
   println(ins.getAddress()+"  "+ins);
   for(Reference r:ins.getReferencesFrom()) println("  REF "+r.getReferenceType()+" -> "+r.getToAddress());
   n++;
  }
 }
}